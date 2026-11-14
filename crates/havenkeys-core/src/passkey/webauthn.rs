//! The authenticator side of WebAuthn Level 3: ES256/ESP256 and
//! EdDSA/Ed25519 keys (see `key.rs`), and the `prf` extension.
//!
//! Composition of audited primitives (`p256`, `ed25519-dalek`, `sha2`,
//! `hmac`); nothing here is new cryptography. Flags and the always-zero counter are explained in
//! docs/crypto.md §Passkeys.

use super::cbor::{self, Value};
use super::key::{Algorithm, KeyMaterial, PrfInput, PrfOutput};
use super::{encode_b64url, B64Url, Passkey, RpContext, CREDENTIAL_ID_LEN};
use crate::crypto::fill_random;
use crate::error::Result;
use sha2::{Digest, Sha256};
use std::fmt;

const FLAG_UP: u8 = 0x01;
const FLAG_UV: u8 = 0x04;
const FLAG_BE: u8 = 0x08;
const FLAG_BS: u8 = 0x10;
const FLAG_AT: u8 = 0x40;
/// HavenKeys' authenticator ID, 5fa9623b-c2ff-4166-92da-c011a036bcb0 (a
/// random UUID v4). It names the provider, so a site can show "HavenKeys"
/// in its passkey list; an all-zero AAGUID is what Apple's passkeys carry,
/// and Google labelled HavenKeys passkeys as Apple's.
const AAGUID: [u8; 16] = [
    0x5f, 0xa9, 0x62, 0x3b, 0xc2, 0xff, 0x41, 0x66, 0x92, 0xda, 0xc0, 0x11, 0xa0, 0x36, 0xbc, 0xb0,
];

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

/// The `prf` extension at creation: give the new passkey a PRF secret, and
/// evaluate it at once when the site sent `eval`.
#[derive(Clone, Copy, Debug)]
pub struct PrfCreate<'a> {
    pub eval: Option<PrfInput<'a>>,
}

/// The `prf` client extension output of a creation.
#[derive(Debug)]
pub struct PrfCreated {
    /// The passkey has a PRF secret. Always true when the site asked.
    pub enabled: bool,
    /// Outputs for `eval` given at creation.
    pub results: Option<PrfOutput>,
}

/// What `navigator.credentials.create()` returns to the site. Public data,
/// except the PRF outputs.
pub struct Registration {
    pub credential_id: Vec<u8>,
    pub client_data_json: Vec<u8>,
    pub authenticator_data: Vec<u8>,
    pub attestation_object: Vec<u8>,
    /// SubjectPublicKeyInfo DER, for `getPublicKey()`.
    pub public_key: Vec<u8>,
    /// COSE algorithm identifier of the key, for `getPublicKeyAlgorithm()`.
    pub public_key_algorithm: i64,
    /// `None` when the site did not ask for `prf`.
    pub prf: Option<PrfCreated>,
}

/// What `navigator.credentials.get()` returns to the site. Public data,
/// except the PRF outputs.
pub struct Assertion {
    pub credential_id: Vec<u8>,
    pub client_data_json: Vec<u8>,
    pub authenticator_data: Vec<u8>,
    /// ASN.1 DER ECDSA signature, or a raw 64-byte Ed25519 signature.
    pub signature: Vec<u8>,
    pub user_handle: Vec<u8>,
    /// `None` when the site sent no PRF input or the passkey has no PRF
    /// secret (made before PRF support, or without `prf`).
    pub prf: Option<PrfOutput>,
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
    alg: Algorithm,
    prf: Option<PrfCreate<'_>>,
) -> Result<(Passkey, Registration)> {
    if let Some(input) = prf.and_then(|p| p.eval) {
        input.check()?;
    }
    let key = KeyMaterial::generate(alg.kind(), prf.is_some())?;
    let mut credential_id = vec![0u8; CREDENTIAL_ID_LEN];
    fill_random(&mut credential_id)?;
    let (cose, public_key) = key.public_key(alg)?;
    let authenticator_data = authenticator_data(&ctx.rp_id, Some((&credential_id, &cose)));
    let prf = match prf {
        None => None,
        Some(p) => Some(PrfCreated {
            enabled: key.has_prf(),
            results: match p.eval {
                Some(input) => key.prf(&input)?,
                None => None,
            },
        }),
    };
    let registration = Registration {
        credential_id: credential_id.clone(),
        client_data_json: client_data_json(Ceremony::Create, challenge, ctx),
        attestation_object: attestation_object_none(&authenticator_data),
        authenticator_data,
        public_key,
        public_key_algorithm: alg.cose(),
        prf,
    };
    let passkey = Passkey {
        credential_id: B64Url(credential_id),
        rp_id: ctx.rp_id.clone(),
        user_handle: B64Url(user.user_handle.to_vec()),
        user_name: user.user_name,
        display_name: user.display_name,
        private_key: key.encode(),
        created_at: now_ms,
        replaced_at: None,
    };
    Ok((passkey, registration))
}

/// Sign an assertion with a stored passkey. The caller has checked that the
/// passkey belongs to `ctx.rp_id`. `client_data_hash`: a privileged
/// browser built `clientDataJSON` itself and sent its hash, which is signed
/// instead; the `client_data_json` returned is then a placeholder the
/// browser replaces.
pub(crate) fn assert(
    passkey: &Passkey,
    ctx: &RpContext,
    challenge: &[u8],
    client_data_hash: Option<&[u8; 32]>,
    prf: Option<&PrfInput<'_>>,
) -> Result<Assertion> {
    let key = KeyMaterial::decode(&passkey.private_key)?;
    let prf = match prf {
        Some(input) => key.prf(input)?,
        None => None,
    };
    let authenticator_data = authenticator_data(&ctx.rp_id, None);
    let client_data_json = client_data_json(Ceremony::Get, challenge, ctx);
    let mut signed = authenticator_data.clone();
    match client_data_hash {
        Some(hash) => signed.extend_from_slice(hash),
        None => signed.extend_from_slice(&Sha256::digest(&client_data_json)),
    }
    let signature = key.sign(&signed)?;
    Ok(Assertion {
        credential_id: passkey.credential_id.0.clone(),
        client_data_json,
        authenticator_data,
        signature,
        user_handle: passkey.user_handle.0.clone(),
        prf,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use crate::secret::SecretBytes;
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
        let (passkey, reg) =
            register(&ctx(), &[5; 32], user(), 42, Algorithm::Es256, None).unwrap();
        assert_eq!(passkey.credential_id.0, reg.credential_id);
        assert_eq!(reg.credential_id.len(), CREDENTIAL_ID_LEN);
        assert_eq!(passkey.rp_id, "github.com");
        assert_eq!(passkey.user_handle.0, vec![1, 2, 3]);
        assert_eq!(passkey.created_at, 42);

        // authenticatorData: rpIdHash, flags AT|BS|BE|UV|UP, counter 0,
        // HavenKeys' AAGUID, credential ID length and ID, then the COSE key.
        let a = &reg.authenticator_data;
        assert_eq!(&a[..32], Sha256::digest(b"github.com").as_slice());
        assert_eq!(a[32], 0x5d);
        assert_eq!(&a[33..37], &[0, 0, 0, 0]);
        assert_eq!(
            &a[37..53],
            uuid::Uuid::parse_str("5fa9623b-c2ff-4166-92da-c011a036bcb0")
                .unwrap()
                .as_bytes()
        );
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
        let (passkey, reg) = register(&ctx(), &[5; 32], user(), 0, Algorithm::Es256, None).unwrap();
        let a = assert(&passkey, &ctx(), &[9; 32], None, None).unwrap();
        assert_eq!(a.credential_id, reg.credential_id);
        assert_eq!(a.user_handle, vec![1, 2, 3]);
        assert_eq!(a.authenticator_data, authenticator_data("github.com", None));
        let vk = VerifyingKey::from_public_key_der(&reg.public_key).unwrap();
        let mut signed = a.authenticator_data.clone();
        signed.extend_from_slice(&Sha256::digest(&a.client_data_json));
        let sig = Signature::from_der(&a.signature).unwrap();
        vk.verify(&signed, &sig).unwrap();
        // A different challenge is a different signed message.
        let b = assert(&passkey, &ctx(), &[8; 32], None, None).unwrap();
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
        let (a, ra) = register(&ctx(), &[5; 32], user(), 0, Algorithm::Es256, None).unwrap();
        let (b, rb) = register(&ctx(), &[5; 32], user(), 0, Algorithm::Es256, None).unwrap();
        assert_ne!(ra.credential_id, rb.credential_id);
        assert_ne!(ra.public_key, rb.public_key);
        assert_ne!(a.private_key.expose(), b.private_key.expose());
    }

    #[test]
    fn a_damaged_key_is_refused_not_used() {
        let (mut passkey, _) =
            register(&ctx(), &[5; 32], user(), 0, Algorithm::Es256, None).unwrap();
        passkey.private_key = SecretBytes::new(vec![0; 32]);
        assert_eq!(
            assert(&passkey, &ctx(), &[1], None, None).err(),
            Some(Error::Corrupted)
        );
    }

    #[test]
    fn a_supplied_client_data_hash_is_what_gets_signed() {
        let (passkey, reg) = register(&ctx(), &[5; 32], user(), 0, Algorithm::Es256, None).unwrap();
        let hash = [0x42; 32];
        let a = assert(&passkey, &ctx(), &[9; 32], Some(&hash), None).unwrap();
        let vk = VerifyingKey::from_public_key_der(&reg.public_key).unwrap();
        let sig = Signature::from_der(&a.signature).unwrap();
        let mut over_hash = a.authenticator_data.clone();
        over_hash.extend_from_slice(&hash);
        vk.verify(&over_hash, &sig).unwrap();
        let mut over_json = a.authenticator_data.clone();
        over_json.extend_from_slice(&Sha256::digest(&a.client_data_json));
        assert!(vk.verify(&over_json, &sig).is_err());
    }

    fn eval<'a>(first: &'a [u8], second: Option<&'a [u8]>) -> PrfInput<'a> {
        PrfInput { first, second }
    }

    #[test]
    fn ed25519_registration_and_assertion_verify_independently() {
        use ring::signature::{UnparsedPublicKey, ED25519};
        for alg in [Algorithm::EdDsa, Algorithm::Ed25519] {
            let (passkey, reg) = register(&ctx(), &[5; 32], user(), 0, alg, None).unwrap();
            assert_eq!(reg.public_key_algorithm, alg.cose());
            assert!(reg.prf.is_none());
            assert_eq!(passkey.private_key.expose().len(), 35, "envelope, no PRF");
            // authenticatorData flags AT|BS|BE|UV|UP, then the OKP COSE key.
            let a = &reg.authenticator_data;
            assert_eq!(a[32], 0x5d);
            let cose: ciborium::Value = ciborium::from_reader(&a[71..]).unwrap();
            let cose = cose.as_map().unwrap();
            let int = |v: &ciborium::Value| i128::from(v.as_integer().unwrap());
            assert_eq!(cose.len(), 4);
            assert_eq!((int(&cose[0].0), int(&cose[0].1)), (1, 1), "kty OKP");
            assert_eq!(
                (int(&cose[1].0), int(&cose[1].1)),
                (3, i128::from(alg.cose()))
            );
            assert_eq!((int(&cose[2].0), int(&cose[2].1)), (-1, 6), "crv Ed25519");
            assert_eq!(int(&cose[3].0), -2);
            let x = cose[3].1.as_bytes().unwrap().clone();
            assert_eq!(x.len(), 32);
            assert_eq!(
                &reg.public_key[12..],
                x.as_slice(),
                "SPKI names the same key"
            );
            // Nothing follows the COSE key: no extension data in authData.
            // 4-entry map header, 3 single-byte pairs (6 bytes), -2 key,
            // 0x58 0x20 byte-string header, 32 bytes.
            assert_eq!(a.len(), 71 + 1 + 6 + 1 + 2 + 32);

            let got = assert(&passkey, &ctx(), &[9; 32], None, None).unwrap();
            assert_eq!(got.signature.len(), 64, "raw EdDSA signature");
            let mut signed = got.authenticator_data.clone();
            signed.extend_from_slice(&Sha256::digest(&got.client_data_json));
            UnparsedPublicKey::new(&ED25519, &x)
                .verify(&signed, &got.signature)
                .unwrap();
        }
    }

    #[test]
    fn esp256_is_an_es256_key_named_minus_9() {
        let (passkey, reg) =
            register(&ctx(), &[5; 32], user(), 0, Algorithm::Esp256, None).unwrap();
        assert_eq!(reg.public_key_algorithm, -9);
        assert_eq!(passkey.private_key.expose().len(), 32, "legacy P-256 form");
        let a = assert(&passkey, &ctx(), &[9; 32], None, None).unwrap();
        let vk = VerifyingKey::from_public_key_der(&reg.public_key).unwrap();
        let mut signed = a.authenticator_data.clone();
        signed.extend_from_slice(&Sha256::digest(&a.client_data_json));
        vk.verify(&signed, &Signature::from_der(&a.signature).unwrap())
            .unwrap();
    }

    #[test]
    fn prf_at_creation_and_at_sign_in_agree() {
        let (passkey, reg) = register(
            &ctx(),
            &[5; 32],
            user(),
            0,
            Algorithm::Es256,
            Some(PrfCreate {
                eval: Some(eval(b"salt-1", Some(b"salt-2"))),
            }),
        )
        .unwrap();
        assert_eq!(
            passkey.private_key.expose().len(),
            67,
            "P-256 + PRF envelope"
        );
        let created = reg.prf.unwrap();
        assert!(created.enabled);
        let at_create = created.results.unwrap();
        let input = eval(b"salt-1", Some(b"salt-2"));
        let a = assert(&passkey, &ctx(), &[9; 32], None, Some(&input)).unwrap();
        let at_get = a.prf.unwrap();
        assert_eq!(at_get.first[..], at_create.first[..]);
        assert_eq!(
            at_get.second.as_ref().unwrap()[..],
            at_create.second.as_ref().unwrap()[..]
        );
        assert_ne!(at_get.first[..], at_get.second.as_ref().unwrap()[..]);
        // The signature is unchanged by PRF.
        let vk = VerifyingKey::from_public_key_der(&reg.public_key).unwrap();
        let mut signed = a.authenticator_data.clone();
        signed.extend_from_slice(&Sha256::digest(&a.client_data_json));
        vk.verify(&signed, &Signature::from_der(&a.signature).unwrap())
            .unwrap();
        // No input, no outputs.
        assert!(assert(&passkey, &ctx(), &[9; 32], None, None)
            .unwrap()
            .prf
            .is_none());
    }

    #[test]
    fn prf_enable_without_eval_and_ed25519_with_prf() {
        let (passkey, reg) = register(
            &ctx(),
            &[5; 32],
            user(),
            0,
            Algorithm::Ed25519,
            Some(PrfCreate { eval: None }),
        )
        .unwrap();
        let created = reg.prf.unwrap();
        assert!(created.enabled);
        assert!(created.results.is_none());
        let a = assert(&passkey, &ctx(), &[1], None, Some(&eval(b"x", None))).unwrap();
        assert!(a.prf.unwrap().second.is_none());
    }

    #[test]
    fn a_passkey_without_a_prf_secret_returns_no_outputs() {
        let (passkey, _) = register(&ctx(), &[5; 32], user(), 0, Algorithm::Es256, None).unwrap();
        let a = assert(&passkey, &ctx(), &[1], None, Some(&eval(b"x", None))).unwrap();
        assert!(a.prf.is_none());
    }

    #[test]
    fn oversized_prf_salts_are_refused() {
        let big = vec![0u8; super::super::key::MAX_PRF_SALT_BYTES + 1];
        assert!(register(
            &ctx(),
            &[5; 32],
            user(),
            0,
            Algorithm::Es256,
            Some(PrfCreate {
                eval: Some(eval(&big, None)),
            }),
        )
        .is_err());
        let (passkey, _) = register(
            &ctx(),
            &[5; 32],
            user(),
            0,
            Algorithm::Es256,
            Some(PrfCreate { eval: None }),
        )
        .unwrap();
        assert!(assert(&passkey, &ctx(), &[1], None, Some(&eval(b"a", Some(&big)))).is_err());
    }
}
