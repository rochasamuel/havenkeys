//! The authenticator side of WebAuthn Level 3, for ES256 only.
//!
//! Composition of audited primitives (`p256`, `sha2`); nothing here is new
//! cryptography. Flags and the always-zero counter are explained in
//! docs/crypto.md §Passkeys.

use super::cbor::{self, Value};
use super::{encode_b64url, B64Url, Passkey, RpContext, COSE_ALG_ES256, CREDENTIAL_ID_LEN};
use crate::crypto::fill_random;
use crate::error::{Error, Result};
use crate::secret::SecretBytes;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use p256::pkcs8::EncodePublicKey;
use sha2::{Digest, Sha256};
use std::fmt;
use zeroize::Zeroizing;

const FLAG_UP: u8 = 0x01;
const FLAG_UV: u8 = 0x04;
const FLAG_BE: u8 = 0x08;
const FLAG_BS: u8 = 0x10;
const FLAG_AT: u8 = 0x40;
const AAGUID: [u8; 16] = [0; 16];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ceremony {
    Create,
    Get,
}

/// The account a site asked to create a passkey for.
pub struct NewUser<'a> {
    pub user_handle: &'a [u8],
    pub user_name: String,
    pub display_name: Option<String>,
}

/// What `navigator.credentials.create()` returns to the site. Public data.
pub struct Registration {
    pub credential_id: Vec<u8>,
    pub client_data_json: Vec<u8>,
    pub authenticator_data: Vec<u8>,
    pub attestation_object: Vec<u8>,
    /// SubjectPublicKeyInfo DER, for `getPublicKey()`.
    pub public_key: Vec<u8>,
}

/// What `navigator.credentials.get()` returns to the site. Public data.
pub struct Assertion {
    pub credential_id: Vec<u8>,
    pub client_data_json: Vec<u8>,
    pub authenticator_data: Vec<u8>,
    /// ASN.1 DER ECDSA signature.
    pub signature: Vec<u8>,
    pub user_handle: Vec<u8>,
}

impl fmt::Debug for Registration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Registration(..)")
    }
}

impl fmt::Debug for Assertion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Assertion(..)")
    }
}

fn json_str(s: &str) -> String {
    // Serializing a &str cannot fail.
    serde_json::to_string(s).unwrap_or_default()
}

/// `clientDataJSON`, built here so the extension cannot choose the origin
/// that gets signed. Key order follows WebAuthn §5.8.1.1 (limited
/// verification algorithm).
pub fn client_data_json(ceremony: Ceremony, challenge: &[u8], ctx: &RpContext) -> Vec<u8> {
    let kind = match ceremony {
        Ceremony::Create => "webauthn.create",
        Ceremony::Get => "webauthn.get",
    };
    let mut s = format!(
        "{{\"type\":{},\"challenge\":{},\"origin\":{},\"crossOrigin\":{}",
        json_str(kind),
        json_str(&encode_b64url(challenge)),
        json_str(&ctx.origin),
        ctx.cross_origin
    );
    if let Some(top) = &ctx.top_origin {
        s.push_str(",\"topOrigin\":");
        s.push_str(&json_str(top));
    }
    s.push('}');
    s.into_bytes()
}

/// `rpIdHash || flags || signCount(0) [|| attestedCredentialData]`.
/// `attested` is `(credential_id, cose_public_key)` on create.
pub fn authenticator_data(rp_id: &str, attested: Option<(&[u8], &[u8])>) -> Vec<u8> {
    let mut out = Sha256::digest(rp_id.as_bytes()).to_vec();
    let mut flags = FLAG_UP | FLAG_UV | FLAG_BE | FLAG_BS;
    if attested.is_some() {
        flags |= FLAG_AT;
    }
    out.push(flags);
    out.extend_from_slice(&0u32.to_be_bytes());
    if let Some((credential_id, cose_key)) = attested {
        out.extend_from_slice(&AAGUID);
        out.extend_from_slice(&(credential_id.len() as u16).to_be_bytes());
        out.extend_from_slice(credential_id);
        out.extend_from_slice(cose_key);
    }
    out
}

/// A new P-256 key from the OS CSPRNG. `from_slice` rejects the (vanishingly
/// rare) scalars outside the curve order; try again rather than bias.
fn new_signing_key() -> Result<(SigningKey, SecretBytes)> {
    for _ in 0..8 {
        let mut bytes = Zeroizing::new([0u8; 32]);
        fill_random(&mut bytes[..])?;
        if let Ok(key) = SigningKey::from_slice(&bytes[..]) {
            return Ok((key, SecretBytes::new(bytes.to_vec())));
        }
    }
    Err(Error::Rng)
}

/// COSE_Key for an EC2 P-256 public key, CTAP2 canonical key order.
fn cose_public_key(vk: &VerifyingKey) -> Vec<u8> {
    let point = vk.to_sec1_point(false);
    let b = point.as_bytes(); // 0x04 || x (32) || y (32)
    cbor::encode(&Value::Map(vec![
        (Value::Int(1), Value::Int(2)),              // kty: EC2
        (Value::Int(3), Value::Int(COSE_ALG_ES256)), // alg: ES256
        (Value::Int(-1), Value::Int(1)),             // crv: P-256
        (Value::Int(-2), Value::Bytes(&b[1..33])),   // x
        (Value::Int(-3), Value::Bytes(&b[33..65])),  // y
    ]))
}

fn attestation_object_none(auth_data: &[u8]) -> Vec<u8> {
    cbor::encode(&Value::Map(vec![
        (Value::Text("fmt"), Value::Text("none")),
        (Value::Text("attStmt"), Value::Map(vec![])),
        (Value::Text("authData"), Value::Bytes(auth_data)),
    ]))
}

/// Create a passkey. Validation of the inputs happens in the caller
/// (`vault.rs`); `ctx` must come from [`super::authorize_rp`].
pub(crate) fn register(
    ctx: &RpContext,
    challenge: &[u8],
    user: NewUser<'_>,
    now_ms: i64,
) -> Result<(Passkey, Registration)> {
    let (key, secret) = new_signing_key()?;
    let mut credential_id = vec![0u8; CREDENTIAL_ID_LEN];
    fill_random(&mut credential_id)?;
    let vk = key.verifying_key();
    let cose = cose_public_key(vk);
    let authenticator_data = authenticator_data(&ctx.rp_id, Some((&credential_id, &cose)));
    let public_key = vk
        .to_public_key_der()
        .map_err(|_| Error::Encryption)?
        .as_bytes()
        .to_vec();
    let registration = Registration {
        credential_id: credential_id.clone(),
        client_data_json: client_data_json(Ceremony::Create, challenge, ctx),
        attestation_object: attestation_object_none(&authenticator_data),
        authenticator_data,
        public_key,
    };
    let passkey = Passkey {
        credential_id: B64Url(credential_id),
        rp_id: ctx.rp_id.clone(),
        user_handle: B64Url(user.user_handle.to_vec()),
        user_name: user.user_name,
        display_name: user.display_name,
        private_key: secret,
        created_at: now_ms,
    };
    Ok((passkey, registration))
}

/// Sign an assertion with a stored passkey. The caller has checked that the
/// passkey belongs to `ctx.rp_id`.
pub(crate) fn assert(passkey: &Passkey, ctx: &RpContext, challenge: &[u8]) -> Result<Assertion> {
    let key = SigningKey::from_slice(passkey.private_key.expose()).map_err(|_| Error::Corrupted)?;
    let authenticator_data = authenticator_data(&ctx.rp_id, None);
    let client_data_json = client_data_json(Ceremony::Get, challenge, ctx);
    let mut signed = authenticator_data.clone();
    signed.extend_from_slice(&Sha256::digest(&client_data_json));
    // `sign` hashes with SHA-256 (ES256) and is deterministic (RFC 6979).
    let signature: Signature = key.sign(&signed);
    Ok(Assertion {
        credential_id: passkey.credential_id.0.clone(),
        client_data_json,
        authenticator_data,
        signature: signature.to_der().as_bytes().to_vec(),
        user_handle: passkey.user_handle.0.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::signature::Verifier;
    use p256::ecdsa::{Signature, VerifyingKey};
    use p256::pkcs8::DecodePublicKey;

    fn ctx() -> RpContext {
        RpContext {
            rp_id: "github.com".into(),
            origin: "https://github.com".into(),
            cross_origin: false,
            top_origin: None,
        }
    }

    fn user() -> NewUser<'static> {
        NewUser {
            user_handle: &[1, 2, 3],
            user_name: "octo".into(),
            display_name: Some("Octo Cat".into()),
        }
    }

    #[test]
    fn client_data_is_exact() {
        assert_eq!(
            client_data_json(Ceremony::Get, &[1, 2, 3], &ctx()),
            br#"{"type":"webauthn.get","challenge":"AQID","origin":"https://github.com","crossOrigin":false}"#
        );
        let framed = RpContext {
            cross_origin: true,
            top_origin: Some("https://github.com".into()),
            origin: "https://gist.github.com".into(),
            ..ctx()
        };
        assert_eq!(
            client_data_json(Ceremony::Create, &[0xff], &framed),
            br#"{"type":"webauthn.create","challenge":"_w","origin":"https://gist.github.com","crossOrigin":true,"topOrigin":"https://github.com"}"#
        );
    }

    #[test]
    fn assertion_authenticator_data_layout() {
        let a = authenticator_data("github.com", None);
        assert_eq!(a.len(), 37);
        assert_eq!(&a[..32], Sha256::digest(b"github.com").as_slice());
        assert_eq!(a[32], 0x1d, "UP|UV|BE|BS");
        assert_eq!(&a[33..], &[0, 0, 0, 0], "counter is always 0");
    }

    #[test]
    fn registration_is_well_formed_and_verifiable() {
        let (passkey, reg) = register(&ctx(), &[5; 32], user(), 42).unwrap();
        assert_eq!(passkey.credential_id.0, reg.credential_id);
        assert_eq!(reg.credential_id.len(), CREDENTIAL_ID_LEN);
        assert_eq!(passkey.rp_id, "github.com");
        assert_eq!(passkey.user_handle.0, vec![1, 2, 3]);
        assert_eq!(passkey.created_at, 42);

        // authenticatorData: rpIdHash, flags AT|BS|BE|UV|UP, counter 0, zero
        // AAGUID, credential ID length and ID, then the COSE key.
        let a = &reg.authenticator_data;
        assert_eq!(&a[..32], Sha256::digest(b"github.com").as_slice());
        assert_eq!(a[32], 0x5d);
        assert_eq!(&a[33..37], &[0, 0, 0, 0]);
        assert_eq!(&a[37..53], &[0; 16]);
        assert_eq!(&a[53..55], &[0, 16]);
        assert_eq!(&a[55..71], reg.credential_id.as_slice());

        // The attestation object decodes (independent CBOR decoder) to
        // {"fmt": "none", "attStmt": {}, "authData": <authenticator data>}.
        let v: ciborium::Value = ciborium::from_reader(reg.attestation_object.as_slice()).unwrap();
        let map = v.as_map().unwrap();
        assert_eq!(map[0].0.as_text(), Some("fmt"));
        assert_eq!(map[0].1.as_text(), Some("none"));
        assert_eq!(map[1].0.as_text(), Some("attStmt"));
        assert!(map[1].1.as_map().unwrap().is_empty());
        assert_eq!(map[2].0.as_text(), Some("authData"));
        assert_eq!(map[2].1.as_bytes().unwrap(), &reg.authenticator_data);

        // The COSE key is {1: 2, 3: -7, -1: 1, -2: x, -3: y} and names the
        // same key as the SPKI public key.
        let cose: ciborium::Value = ciborium::from_reader(&a[71..]).unwrap();
        let cose = cose.as_map().unwrap();
        let int = |v: &ciborium::Value| i128::from(v.as_integer().unwrap());
        let keys: Vec<i128> = cose.iter().map(|(k, _)| int(k)).collect();
        assert_eq!(keys, vec![1, 3, -1, -2, -3]);
        assert_eq!(int(&cose[0].1), 2);
        assert_eq!(int(&cose[1].1), -7);
        assert_eq!(int(&cose[2].1), 1);
        let mut sec1 = vec![4u8];
        sec1.extend_from_slice(cose[3].1.as_bytes().unwrap());
        sec1.extend_from_slice(cose[4].1.as_bytes().unwrap());
        let from_cose = VerifyingKey::from_sec1_bytes(&sec1).unwrap();
        let from_spki = VerifyingKey::from_public_key_der(&reg.public_key).unwrap();
        assert_eq!(from_cose, from_spki);

        assert_eq!(
            reg.client_data_json,
            client_data_json(Ceremony::Create, &[5; 32], &ctx())
        );
    }

    #[test]
    fn assertion_signature_verifies() {
        let (passkey, reg) = register(&ctx(), &[5; 32], user(), 0).unwrap();
        let a = assert(&passkey, &ctx(), &[9; 32]).unwrap();
        assert_eq!(a.credential_id, reg.credential_id);
        assert_eq!(a.user_handle, vec![1, 2, 3]);
        assert_eq!(a.authenticator_data, authenticator_data("github.com", None));
        let vk = VerifyingKey::from_public_key_der(&reg.public_key).unwrap();
        let mut signed = a.authenticator_data.clone();
        signed.extend_from_slice(&Sha256::digest(&a.client_data_json));
        let sig = Signature::from_der(&a.signature).unwrap();
        vk.verify(&signed, &sig).unwrap();
        // A different challenge is a different signed message.
        let b = assert(&passkey, &ctx(), &[8; 32]).unwrap();
        let mut other = b.authenticator_data.clone();
        other.extend_from_slice(&Sha256::digest(&b.client_data_json));
        assert!(vk
            .verify(&signed, &Signature::from_der(&b.signature).unwrap())
            .is_err());
        assert!(vk
            .verify(&other, &Signature::from_der(&b.signature).unwrap())
            .is_ok());
    }

    #[test]
    fn every_registration_gets_fresh_keys() {
        let (a, ra) = register(&ctx(), &[5; 32], user(), 0).unwrap();
        let (b, rb) = register(&ctx(), &[5; 32], user(), 0).unwrap();
        assert_ne!(ra.credential_id, rb.credential_id);
        assert_ne!(ra.public_key, rb.public_key);
        assert_ne!(a.private_key.expose(), b.private_key.expose());
    }

    #[test]
    fn a_damaged_key_is_refused_not_used() {
        let (mut passkey, _) = register(&ctx(), &[5; 32], user(), 0).unwrap();
        passkey.private_key = SecretBytes::new(vec![0; 32]);
        assert_eq!(assert(&passkey, &ctx(), &[1]).err(), Some(Error::Corrupted));
    }
}
