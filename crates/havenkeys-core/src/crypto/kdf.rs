//! Argon2id password-based key derivation.

use crate::crypto::fill_random;
use crate::crypto::keys::{Key256, KEY_LEN};
use crate::error::{Error, Result};
use crate::secret::SecretString;
use argon2::{Algorithm, Argon2, Params, Version};
use data_encoding::BASE64;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

pub const SALT_LEN: usize = 16;

/// Defaults: 128 MiB, t=4, p=4 — above RFC 9106 §4's second recommended
/// option (64 MiB, t=3), chosen from `examples/kdf_bench.rs` measurements
/// (see docs/crypto.md) to cost roughly 0.3–1 s per unlock on laptops.
pub const DEFAULT_MEMORY_KIB: u32 = 128 * 1024;
pub const DEFAULT_ITERATIONS: u32 = 4;
pub const DEFAULT_PARALLELISM: u32 = 4;

// Accepted ranges when reading a header from disk. The floor (OWASP minimum
// for Argon2id) rejects headers tampered to make guessing cheap; the ceiling
// stops a malicious header from exhausting memory or CPU.
pub const MIN_MEMORY_KIB: u32 = 19 * 1024;
pub const MAX_MEMORY_KIB: u32 = 1024 * 1024;
pub const MIN_ITERATIONS: u32 = 2;
pub const MAX_ITERATIONS: u32 = 16;
pub const MIN_PARALLELISM: u32 = 1;
pub const MAX_PARALLELISM: u32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KdfAlgorithm {
    Argon2id,
}

/// KDF descriptor stored (in plaintext) in the vault header.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KdfParams {
    pub algorithm: KdfAlgorithm,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    #[serde(serialize_with = "ser_salt", deserialize_with = "de_salt")]
    pub salt: [u8; SALT_LEN],
}

impl KdfParams {
    /// Default cost with a fresh random salt.
    pub fn generate() -> Result<Self> {
        Self::with_cost(DEFAULT_MEMORY_KIB, DEFAULT_ITERATIONS, DEFAULT_PARALLELISM)
    }

    /// Custom cost with a fresh random salt. Validated against the accepted ranges.
    pub fn with_cost(memory_kib: u32, iterations: u32, parallelism: u32) -> Result<Self> {
        let mut salt = [0u8; SALT_LEN];
        fill_random(&mut salt)?;
        let params = Self {
            algorithm: KdfAlgorithm::Argon2id,
            memory_kib,
            iterations,
            parallelism,
            salt,
        };
        params.validate()?;
        Ok(params)
    }

    pub fn validate(&self) -> Result<()> {
        let ok = (MIN_MEMORY_KIB..=MAX_MEMORY_KIB).contains(&self.memory_kib)
            && (MIN_ITERATIONS..=MAX_ITERATIONS).contains(&self.iterations)
            && (MIN_PARALLELISM..=MAX_PARALLELISM).contains(&self.parallelism);
        if ok {
            Ok(())
        } else {
            Err(Error::Corrupted)
        }
    }
}

/// Derive the 256-bit master key from the master password.
///
/// The password is Unicode-normalized (NFC) first so the same visible
/// password typed through different input methods yields the same key.
pub fn derive_master_key(password: &SecretString, params: &KdfParams) -> Result<Key256> {
    params.validate()?;
    let argon_params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(KEY_LEN),
    )
    .map_err(|_| Error::Kdf)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);

    let normalized: Zeroizing<String> = Zeroizing::new(password.expose().nfc().collect());
    let mut out = [0u8; KEY_LEN];
    let result = argon.hash_password_into(normalized.as_bytes(), &params.salt, &mut out);
    let key = Key256::from_bytes(out);
    zeroize::Zeroize::zeroize(&mut out);
    result.map_err(|_| Error::Kdf)?;
    Ok(key)
}

fn ser_salt<S: Serializer>(salt: &[u8; SALT_LEN], s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_str(&BASE64.encode(salt))
}

fn de_salt<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<[u8; SALT_LEN], D::Error> {
    let encoded = String::deserialize(d)?;
    let bytes = BASE64
        .decode(encoded.as_bytes())
        .map_err(|_| serde::de::Error::custom("invalid salt"))?;
    bytes
        .try_into()
        .map_err(|_| serde::de::Error::custom("invalid salt length"))
}

#[cfg(test)]
pub(crate) fn test_params() -> KdfParams {
    // The cheapest parameters the validator accepts; keeps tests fast.
    KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_same_password_and_salt() {
        let p = test_params();
        let a = derive_master_key(&"correct horse".into(), &p).unwrap();
        let b = derive_master_key(&"correct horse".into(), &p).unwrap();
        assert_eq!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn different_password_gives_different_key() {
        let p = test_params();
        let a = derive_master_key(&"correct horse".into(), &p).unwrap();
        let b = derive_master_key(&"correct horsf".into(), &p).unwrap();
        assert_ne!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn different_salt_gives_different_key() {
        let a = derive_master_key(&"pw-pw-pw-pw".into(), &test_params()).unwrap();
        let b = derive_master_key(&"pw-pw-pw-pw".into(), &test_params()).unwrap();
        assert_ne!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn unicode_normalization_applies() {
        let p = test_params();
        // "é" precomposed vs. "e" + combining acute accent.
        let a = derive_master_key(&"caf\u{00e9}-password".into(), &p).unwrap();
        let b = derive_master_key(&"cafe\u{0301}-password".into(), &p).unwrap();
        assert_eq!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn matches_argon2id_reference_output() {
        // Guard against silently changing algorithm/version/params mapping:
        // compare with a direct call to the argon2 crate.
        let p = test_params();
        let ours = derive_master_key(&"reference".into(), &p).unwrap();
        let mut direct = [0u8; 32];
        Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(p.memory_kib, p.iterations, p.parallelism, Some(32)).unwrap(),
        )
        .hash_password_into(b"reference", &p.salt, &mut direct)
        .unwrap();
        assert_eq!(ours.as_bytes(), &direct);
    }

    #[test]
    fn rejects_out_of_range_params() {
        let mut p = test_params();
        p.memory_kib = 1024;
        assert_eq!(p.validate(), Err(Error::Corrupted));
        let mut p = test_params();
        p.iterations = 1;
        assert!(p.validate().is_err());
        let mut p = test_params();
        p.parallelism = 0;
        assert!(p.validate().is_err());
        let mut p = test_params();
        p.memory_kib = MAX_MEMORY_KIB + 1;
        assert!(p.validate().is_err());
        assert!(derive_master_key(&"x".into(), &p).is_err());
    }

    #[test]
    fn params_serde_round_trip_and_strictness() {
        let p = test_params();
        let json = serde_json::to_string(&p).unwrap();
        let back: KdfParams = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
        let extra = json.replacen('{', "{\"extra\":1,", 1);
        assert!(serde_json::from_str::<KdfParams>(&extra).is_err());
        let bad_alg = json.replace("argon2id", "pbkdf2");
        assert!(serde_json::from_str::<KdfParams>(&bad_alg).is_err());
    }
}
