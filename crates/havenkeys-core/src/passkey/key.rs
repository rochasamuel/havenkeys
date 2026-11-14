//! A passkey's key material: which algorithm, the private key, and the
//! optional PRF secret (spec 2026-11-14-passkey-parity §4–§5).
//!
//! Everything lives in the one field every HavenKeys version keeps byte for
//! byte when it re-saves a login: `Passkey.private_key`. Versions before
//! this spec read that field as a P-256 scalar and drop any field of
//! `Passkey` they do not know, so the format is chosen for them:
//!
//! * exactly 32 bytes: a P-256 scalar, no PRF secret (every passkey made
//!   before this spec, and every ES256 passkey made without `prf`);
//! * otherwise a versioned envelope, `0x01 || kind || flags || key(32)
//!   [|| prf_secret(32)]`, 35 or 67 bytes. An older version hands it to
//!   `p256::SecretKey::from_slice`, which accepts only 24–32 bytes, so it
//!   refuses that one passkey as damaged and never signs with an Ed25519
//!   seed as if it were a P-256 scalar; and since it keeps the bytes as they
//!   are, an edit there loses neither the key nor the PRF secret.
//!
//! Composition of audited primitives only (`p256`, `ed25519-dalek`, `sha2`,
//! `hmac`). Nothing here is new cryptography.

use super::cbor::{self, Value};
use crate::crypto::fill_random;
use crate::error::{Error, Result};
use crate::secret::SecretBytes;
use ed25519_dalek::pkcs8::EncodePublicKey as _;
use hmac::{KeyInit, Mac};
use p256::ecdsa::signature::Signer;
use sha2::{Digest, Sha256};
use std::fmt;
use zeroize::Zeroizing;

/// COSE algorithm identifiers (IANA COSE Algorithms registry).
pub const COSE_ALG_ES256: i64 = -7;
/// ECDSA P-256 with SHA-256, fully specified (RFC 9864). Same signatures as ES256.
pub const COSE_ALG_ESP256: i64 = -9;
/// EdDSA (RFC 8152); with WebAuthn, always Ed25519.
pub const COSE_ALG_EDDSA: i64 = -8;
/// Ed25519, fully specified (RFC 9864).
pub const COSE_ALG_ED25519: i64 = -19;

/// Bytes of one PRF secret and of one PRF output.
pub const PRF_SECRET_LEN: usize = 32;
/// Largest PRF salt (`eval.first` / `eval.second`) accepted, in bytes.
/// WebAuthn sets no bound; sites send 32.
pub const MAX_PRF_SALT_BYTES: usize = 1024;
/// Most `pubKeyCredParams` entries looked at.
pub const MAX_ALGORITHMS: usize = 16;

const ENVELOPE_V1: u8 = 1;
const KIND_P256: u8 = 1;
const KIND_ED25519: u8 = 2;
const FLAG_PRF: u8 = 0x01;
const LEGACY_LEN: usize = 32;
const KEY_LEN: usize = 32;
const HEADER_LEN: usize = 3;

/// The algorithm a new passkey is created with: the COSE identifier the
/// site asked for, which decides the key type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Algorithm {
    Es256,
    Esp256,
    EdDsa,
    Ed25519,
}

impl Algorithm {
    pub fn from_cose(alg: i64) -> Option<Self> {
        match alg {
            COSE_ALG_ES256 => Some(Self::Es256),
            COSE_ALG_ESP256 => Some(Self::Esp256),
            COSE_ALG_EDDSA => Some(Self::EdDsa),
            COSE_ALG_ED25519 => Some(Self::Ed25519),
            _ => None,
        }
    }

    pub fn cose(self) -> i64 {
        match self {
            Self::Es256 => COSE_ALG_ES256,
            Self::Esp256 => COSE_ALG_ESP256,
            Self::EdDsa => COSE_ALG_EDDSA,
            Self::Ed25519 => COSE_ALG_ED25519,
        }
    }

    pub(crate) fn kind(self) -> KeyKind {
        match self {
            Self::Es256 | Self::Esp256 => KeyKind::P256,
            Self::EdDsa | Self::Ed25519 => KeyKind::Ed25519,
        }
    }
}

/// The first algorithm in the site's `pubKeyCredParams` order that
/// HavenKeys supports. An empty list means WebAuthn's default, ES256
/// (WebAuthn L3 §5.4 `pubKeyCredParams`). `None`: nothing supported.
pub fn choose_algorithm(params: &[i64]) -> Option<Algorithm> {
    if params.is_empty() {
        return Some(Algorithm::Es256);
    }
    params
        .iter()
        .take(MAX_ALGORITHMS)
        .find_map(|&a| Algorithm::from_cose(a))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyKind {
    P256,
    Ed25519,
}

/// A passkey's decoded key material. Zeroized on drop; never printed.
pub(crate) struct KeyMaterial {
    pub kind: KeyKind,
    key: Zeroizing<[u8; KEY_LEN]>,
    prf: Option<Zeroizing<[u8; PRF_SECRET_LEN]>>,
}

impl fmt::Debug for KeyMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KeyMaterial(<redacted>)")
    }
}

fn array32(bytes: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let mut out = Zeroizing::new([0u8; 32]);
    if bytes.len() != 32 {
        return Err(Error::Corrupted);
    }
    out.copy_from_slice(bytes);
    Ok(out)
}

impl KeyMaterial {
    /// Decode `Passkey.private_key`. Anything but the two shapes above, an
    /// unknown version, kind or flag, or a key the curve refuses, is
    /// `Corrupted`.
    pub fn decode(stored: &SecretBytes) -> Result<Self> {
        let b = stored.expose();
        let m = if b.len() == LEGACY_LEN {
            Self {
                kind: KeyKind::P256,
                key: array32(b)?,
                prf: None,
            }
        } else {
            if b.len() < HEADER_LEN || b[0] != ENVELOPE_V1 {
                return Err(Error::Corrupted);
            }
            let kind = match b[1] {
                KIND_P256 => KeyKind::P256,
                KIND_ED25519 => KeyKind::Ed25519,
                _ => return Err(Error::Corrupted),
            };
            let flags = b[2];
            if flags & !FLAG_PRF != 0 {
                return Err(Error::Corrupted);
            }
            let has_prf = flags & FLAG_PRF != 0;
            let want = HEADER_LEN + KEY_LEN + if has_prf { PRF_SECRET_LEN } else { 0 };
            if b.len() != want {
                return Err(Error::Corrupted);
            }
            let key = array32(&b[HEADER_LEN..HEADER_LEN + KEY_LEN])?;
            let prf = if has_prf {
                Some(array32(&b[HEADER_LEN + KEY_LEN..])?)
            } else {
                None
            };
            Self { kind, key, prf }
        };
        if m.kind == KeyKind::P256 {
            // Rejects zero and scalars outside the curve order.
            p256::ecdsa::SigningKey::from_slice(&m.key[..]).map_err(|_| Error::Corrupted)?;
        }
        Ok(m)
    }

    /// Encode for `Passkey.private_key`. A P-256 key without a PRF secret
    /// keeps the 32-byte form every older version reads.
    pub fn encode(&self) -> SecretBytes {
        if self.kind == KeyKind::P256 && self.prf.is_none() {
            return SecretBytes::new(self.key.to_vec());
        }
        let mut out = Vec::with_capacity(HEADER_LEN + KEY_LEN + PRF_SECRET_LEN);
        out.push(ENVELOPE_V1);
        out.push(match self.kind {
            KeyKind::P256 => KIND_P256,
            KeyKind::Ed25519 => KIND_ED25519,
        });
        out.push(if self.prf.is_some() { FLAG_PRF } else { 0 });
        out.extend_from_slice(&self.key[..]);
        if let Some(p) = &self.prf {
            out.extend_from_slice(&p[..]);
        }
        SecretBytes::new(out)
    }

    /// A new key of `kind` from the OS CSPRNG, and a PRF secret when asked.
    pub fn generate(kind: KeyKind, with_prf: bool) -> Result<Self> {
        let key = match kind {
            KeyKind::P256 => new_p256_scalar()?,
            KeyKind::Ed25519 => {
                // RFC 8032 §5.1.5: the private key is 32 random bytes.
                let mut seed = Zeroizing::new([0u8; KEY_LEN]);
                fill_random(&mut seed[..])?;
                seed
            }
        };
        let prf = if with_prf {
            let mut s = Zeroizing::new([0u8; PRF_SECRET_LEN]);
            fill_random(&mut s[..])?;
            Some(s)
        } else {
            None
        };
        Ok(Self { kind, key, prf })
    }

    pub fn has_prf(&self) -> bool {
        self.prf.is_some()
    }

    /// The COSE_Key (CTAP2 canonical key order) naming `alg`, and the
    /// SubjectPublicKeyInfo DER for `getPublicKey()`.
    pub fn public_key(&self, alg: Algorithm) -> Result<(Vec<u8>, Vec<u8>)> {
        match self.kind {
            KeyKind::P256 => {
                let sk = p256::ecdsa::SigningKey::from_slice(&self.key[..])
                    .map_err(|_| Error::Corrupted)?;
                let vk = sk.verifying_key();
                let point = vk.to_sec1_point(false);
                let b = point.as_bytes(); // 0x04 || x (32) || y (32)
                let cose = cbor::encode(&Value::Map(vec![
                    (Value::Int(1), Value::Int(2)),             // kty: EC2
                    (Value::Int(3), Value::Int(alg.cose())),    // alg: ES256 / ESP256
                    (Value::Int(-1), Value::Int(1)),            // crv: P-256
                    (Value::Int(-2), Value::Bytes(&b[1..33])),  // x
                    (Value::Int(-3), Value::Bytes(&b[33..65])), // y
                ]));
                let spki = vk
                    .to_public_key_der()
                    .map_err(|_| Error::Encryption)?
                    .as_bytes()
                    .to_vec();
                Ok((cose, spki))
            }
            KeyKind::Ed25519 => {
                let vk = ed25519_dalek::SigningKey::from_bytes(&self.key).verifying_key();
                let x = vk.to_bytes();
                // RFC 9053 §7.2 OKP key; RFC 8152 §13.2 / RFC 9053 Table 18.
                let cose = cbor::encode(&Value::Map(vec![
                    (Value::Int(1), Value::Int(1)),          // kty: OKP
                    (Value::Int(3), Value::Int(alg.cose())), // alg: EdDSA / Ed25519
                    (Value::Int(-1), Value::Int(6)),         // crv: Ed25519
                    (Value::Int(-2), Value::Bytes(&x)),      // x
                ]));
                let spki = vk
                    .to_public_key_der()
                    .map_err(|_| Error::Encryption)?
                    .as_bytes()
                    .to_vec();
                Ok((cose, spki))
            }
        }
    }

    /// Sign `message` (authenticatorData || clientDataHash). P-256: ASN.1
    /// DER ECDSA over SHA-256, deterministic (RFC 6979). Ed25519: the raw
    /// 64-byte PureEdDSA signature (RFC 8032), as WebAuthn §6.5.6 asks.
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>> {
        match self.kind {
            KeyKind::P256 => {
                let sk = p256::ecdsa::SigningKey::from_slice(&self.key[..])
                    .map_err(|_| Error::Corrupted)?;
                let sig: p256::ecdsa::Signature = sk.sign(message);
                Ok(sig.to_der().as_bytes().to_vec())
            }
            KeyKind::Ed25519 => {
                let sk = ed25519_dalek::SigningKey::from_bytes(&self.key);
                let sig: ed25519_dalek::Signature = sk.sign(message);
                Ok(sig.to_bytes().to_vec())
            }
        }
    }

    /// The PRF outputs for `input` (WebAuthn L3 §10.1.4, CTAP 2.2 §12.5):
    /// `salt' = SHA-256("WebAuthn PRF" || 0x00 || salt)` and
    /// `output = HMAC-SHA-256(secret, salt')`. `None` without a PRF secret.
    pub fn prf(&self, input: &PrfInput<'_>) -> Result<Option<PrfOutput>> {
        let Some(secret) = &self.prf else {
            return Ok(None);
        };
        input.check()?;
        let first = prf_eval(secret, input.first);
        let second = input.second.map(|s| prf_eval(secret, s));
        Ok(Some(PrfOutput { first, second }))
    }
}

/// A new P-256 scalar from the OS CSPRNG. `from_slice` rejects the
/// (vanishingly rare) values outside the curve order; try again rather
/// than bias.
fn new_p256_scalar() -> Result<Zeroizing<[u8; KEY_LEN]>> {
    for _ in 0..8 {
        let mut bytes = Zeroizing::new([0u8; KEY_LEN]);
        fill_random(&mut bytes[..])?;
        if p256::ecdsa::SigningKey::from_slice(&bytes[..]).is_ok() {
            return Ok(bytes);
        }
    }
    Err(Error::Rng)
}

fn prf_eval(secret: &[u8; PRF_SECRET_LEN], salt: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut h = Sha256::new();
    h.update(b"WebAuthn PRF");
    h.update([0u8]);
    h.update(salt);
    let salted = h.finalize();
    // HMAC accepts any key length; 32 bytes cannot fail.
    let mut mac = <hmac::Hmac<Sha256> as KeyInit>::new_from_slice(secret)
        .expect("HMAC takes keys of any length");
    mac.update(&salted);
    let mut out = Zeroizing::new([0u8; 32]);
    out.copy_from_slice(&mac.finalize().into_bytes());
    out
}

/// A site's PRF inputs for one credential (`eval` or its
/// `evalByCredential` entry), as the site sent them (not yet hashed).
#[derive(Clone, Copy)]
pub struct PrfInput<'a> {
    pub first: &'a [u8],
    pub second: Option<&'a [u8]>,
}

impl PrfInput<'_> {
    pub(crate) fn check(&self) -> Result<()> {
        let ok = |s: &[u8]| s.len() <= MAX_PRF_SALT_BYTES;
        if ok(self.first) && self.second.is_none_or(ok) {
            Ok(())
        } else {
            Err(Error::InvalidInput("invalid PRF input"))
        }
    }
}

impl fmt::Debug for PrfInput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PrfInput(..)")
    }
}

/// PRF outputs: secrets the site derives keys from. Zeroized on drop,
/// never printed.
pub struct PrfOutput {
    pub first: Zeroizing<[u8; 32]>,
    pub second: Option<Zeroizing<[u8; 32]>>,
}

impl fmt::Debug for PrfOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PrfOutput(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        data_encoding::HEXLOWER.decode(s.as_bytes()).unwrap()
    }

    #[test]
    fn algorithm_choice_follows_the_sites_order() {
        assert_eq!(
            choose_algorithm(&[]),
            Some(Algorithm::Es256),
            "spec default"
        );
        assert_eq!(choose_algorithm(&[-7, -8]), Some(Algorithm::Es256));
        assert_eq!(choose_algorithm(&[-8, -7]), Some(Algorithm::EdDsa));
        assert_eq!(choose_algorithm(&[-257, -19, -7]), Some(Algorithm::Ed25519));
        assert_eq!(choose_algorithm(&[-9]), Some(Algorithm::Esp256));
        assert_eq!(choose_algorithm(&[-257, -37]), None, "RSA only");
        // ML-DSA (-48/-49/-50) is not supported.
        assert_eq!(choose_algorithm(&[-48, -49, -50]), None);
        // Only the first MAX_ALGORITHMS entries are looked at.
        let mut long = vec![-257; MAX_ALGORITHMS];
        long.push(-7);
        assert_eq!(choose_algorithm(&long), None);
    }

    #[test]
    fn legacy_32_byte_keys_stay_32_bytes() {
        let m = KeyMaterial::generate(KeyKind::P256, false).unwrap();
        let enc = m.encode();
        assert_eq!(enc.expose().len(), 32);
        let back = KeyMaterial::decode(&enc).unwrap();
        assert_eq!(back.kind, KeyKind::P256);
        assert!(!back.has_prf());
        assert_eq!(back.key[..], m.key[..]);
    }

    #[test]
    fn envelopes_round_trip() {
        for (kind, prf, len) in [
            (KeyKind::P256, true, 67),
            (KeyKind::Ed25519, false, 35),
            (KeyKind::Ed25519, true, 67),
        ] {
            let m = KeyMaterial::generate(kind, prf).unwrap();
            let enc = m.encode();
            assert_eq!(enc.expose().len(), len);
            assert_eq!(enc.expose()[0], 1);
            let back = KeyMaterial::decode(&enc).unwrap();
            assert_eq!(back.kind, kind);
            assert_eq!(back.has_prf(), prf);
            assert_eq!(back.key[..], m.key[..]);
            assert_eq!(
                back.prf.as_ref().map(|p| p.to_vec()),
                m.prf.as_ref().map(|p| p.to_vec())
            );
        }
    }

    #[test]
    fn damaged_envelopes_are_refused() {
        let good = KeyMaterial::generate(KeyKind::Ed25519, true)
            .unwrap()
            .encode();
        let g = good.expose().to_vec();
        let mut cases: Vec<Vec<u8>> = vec![
            vec![],
            vec![1, 2],
            vec![0; 32],                   // zero P-256 scalar
            vec![0xff; 32],                // above the curve order
            g[..66].to_vec(),              // truncated
            [g.clone(), vec![0]].concat(), // trailing byte
        ];
        let mut bad_version = g.clone();
        bad_version[0] = 2;
        cases.push(bad_version);
        let mut bad_kind = g.clone();
        bad_kind[1] = 3;
        cases.push(bad_kind);
        let mut bad_flag = g.clone();
        bad_flag[2] = 0x03;
        cases.push(bad_flag);
        let mut no_flag_but_long = g.clone();
        no_flag_but_long[2] = 0;
        cases.push(no_flag_but_long);
        for c in cases {
            assert!(
                KeyMaterial::decode(&SecretBytes::new(c.clone())).is_err(),
                "{} bytes",
                c.len()
            );
        }
    }

    /// What every HavenKeys version before this spec does with
    /// `private_key`: `p256` refuses a key that is not 24–32 bytes. The
    /// envelope is never in that range, so an older version cannot sign
    /// with an Ed25519 seed as a P-256 scalar.
    #[test]
    fn older_versions_refuse_every_envelope() {
        for (kind, prf) in [
            (KeyKind::P256, true),
            (KeyKind::Ed25519, false),
            (KeyKind::Ed25519, true),
        ] {
            let enc = KeyMaterial::generate(kind, prf).unwrap().encode();
            assert!(p256::ecdsa::SigningKey::from_slice(enc.expose()).is_err());
            assert!(enc.expose().len() > 32);
        }
    }

    #[test]
    fn ed25519_matches_rfc8032_test_1() {
        // RFC 8032 §7.1, TEST 1 (empty message).
        let seed = hex("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
        let mut b = vec![ENVELOPE_V1, KIND_ED25519, 0];
        b.extend_from_slice(&seed);
        let m = KeyMaterial::decode(&SecretBytes::new(b)).unwrap();
        let (cose, spki) = m.public_key(Algorithm::EdDsa).unwrap();
        let public = hex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a");
        assert_eq!(&spki[spki.len() - 32..], public.as_slice());
        // SPKI: SEQUENCE { SEQUENCE { OID 1.3.101.112 }, BIT STRING }.
        assert_eq!(&spki[..12], hex("302a300506032b6570032100").as_slice());
        assert_eq!(
            m.sign(b"").unwrap(),
            hex("e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b")
        );
        // COSE_Key {1: 1, 3: -8, -1: 6, -2: x}, decoded independently.
        let v: ciborium::Value = ciborium::from_reader(cose.as_slice()).unwrap();
        let map = v.as_map().unwrap();
        let int = |v: &ciborium::Value| i128::from(v.as_integer().unwrap());
        let keys: Vec<i128> = map.iter().map(|(k, _)| int(k)).collect();
        assert_eq!(keys, vec![1, 3, -1, -2]);
        assert_eq!(int(&map[0].1), 1, "kty OKP");
        assert_eq!(int(&map[1].1), -8, "alg EdDSA");
        assert_eq!(int(&map[2].1), 6, "crv Ed25519");
        assert_eq!(map[3].1.as_bytes().unwrap(), &public);
        let (cose19, _) = m.public_key(Algorithm::Ed25519).unwrap();
        let v: ciborium::Value = ciborium::from_reader(cose19.as_slice()).unwrap();
        assert_eq!(int(&v.as_map().unwrap()[1].1), -19);
    }

    #[test]
    fn signatures_verify_with_an_independent_library() {
        use ring::signature::{UnparsedPublicKey, ECDSA_P256_SHA256_ASN1, ED25519};
        let msg = b"authenticatorData || clientDataHash";
        let ed = KeyMaterial::generate(KeyKind::Ed25519, false).unwrap();
        let (_, spki) = ed.public_key(Algorithm::Ed25519).unwrap();
        let sig = ed.sign(msg).unwrap();
        assert_eq!(sig.len(), 64);
        let pk = UnparsedPublicKey::new(&ED25519, &spki[12..]);
        pk.verify(msg, &sig).unwrap();
        assert!(pk.verify(b"other", &sig).is_err());

        let ec = KeyMaterial::generate(KeyKind::P256, true).unwrap();
        let (cose, _) = ec.public_key(Algorithm::Esp256).unwrap();
        let v: ciborium::Value = ciborium::from_reader(cose.as_slice()).unwrap();
        let map = v.as_map().unwrap();
        assert_eq!(i128::from(map[1].1.as_integer().unwrap()), -9);
        let mut point = vec![4u8];
        point.extend_from_slice(map[3].1.as_bytes().unwrap());
        point.extend_from_slice(map[4].1.as_bytes().unwrap());
        let sig = ec.sign(msg).unwrap();
        UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, &point)
            .verify(msg, &sig)
            .unwrap();
    }

    #[test]
    fn prf_matches_the_webauthn_definition() {
        // Computed here from the definition with `ring` (independent of
        // `sha2`/`hmac`): HMAC-SHA-256(secret, SHA-256("WebAuthn PRF" || 0 || salt)).
        use ring::{digest, hmac as rhmac};
        let secret = [0x42u8; 32];
        let mut b = vec![ENVELOPE_V1, KIND_P256, FLAG_PRF];
        b.extend_from_slice(&[7u8; 32]);
        b.extend_from_slice(&secret);
        let m = KeyMaterial::decode(&SecretBytes::new(b)).unwrap();
        let expect = |salt: &[u8]| {
            let mut ctx = digest::Context::new(&digest::SHA256);
            ctx.update(b"WebAuthn PRF\x00");
            ctx.update(salt);
            let salted = ctx.finish();
            let key = rhmac::Key::new(rhmac::HMAC_SHA256, &secret);
            rhmac::sign(&key, salted.as_ref()).as_ref().to_vec()
        };
        let out = m
            .prf(&PrfInput {
                first: b"first salt",
                second: Some(b""),
            })
            .unwrap()
            .unwrap();
        assert_eq!(out.first.to_vec(), expect(b"first salt"));
        assert_eq!(
            out.first.to_vec(),
            hex("b3263aed0281b6d3b7bc87e700289adfb1ef86bb190853ba151493c1d8e92c4b")
        );
        assert_eq!(out.second.unwrap().to_vec(), expect(b""));
        // A fixed known answer as well, so a change to the definition shows.
        let out = m
            .prf(&PrfInput {
                first: &[0u8; 32],
                second: None,
            })
            .unwrap()
            .unwrap();
        assert_eq!(out.first.to_vec(), expect(&[0u8; 32]));
        // The same value from Python's hashlib/hmac.
        assert_eq!(
            out.first.to_vec(),
            hex("5b49fea55a35389ba63083a2ea8ef03f6847b91bb171a14b9441a0acaf5223d8")
        );
        assert!(out.second.is_none());

        // No secret: no outputs. Oversized salts are refused.
        let legacy = KeyMaterial::generate(KeyKind::P256, false).unwrap();
        assert!(legacy
            .prf(&PrfInput {
                first: b"x",
                second: None
            })
            .unwrap()
            .is_none());
        let big = vec![0u8; MAX_PRF_SALT_BYTES + 1];
        assert!(m
            .prf(&PrfInput {
                first: &big,
                second: None
            })
            .is_err());
    }

    #[test]
    fn prf_secrets_are_per_credential() {
        let a = KeyMaterial::generate(KeyKind::P256, true).unwrap();
        let b = KeyMaterial::generate(KeyKind::P256, true).unwrap();
        let input = PrfInput {
            first: b"s",
            second: None,
        };
        assert_ne!(
            a.prf(&input).unwrap().unwrap().first.to_vec(),
            b.prf(&input).unwrap().unwrap().first.to_vec()
        );
        assert_eq!(
            a.prf(&input).unwrap().unwrap().first.to_vec(),
            a.prf(&input).unwrap().unwrap().first.to_vec()
        );
    }

    #[test]
    fn key_material_debug_is_redacted() {
        let m = KeyMaterial::generate(KeyKind::Ed25519, true).unwrap();
        assert_eq!(format!("{m:?}"), "KeyMaterial(<redacted>)");
    }
}
