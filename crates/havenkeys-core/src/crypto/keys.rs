//! 256-bit key material and HKDF key separation.

use crate::crypto::fill_random;
use crate::crypto::secret_key::{SecretKey, SECRET_KEY_LEN};
use crate::error::{Error, Result};
use hkdf::Hkdf;
use sha2::Sha256;
use std::fmt;
use uuid::Uuid;
use zeroize::Zeroizing;

pub const KEY_LEN: usize = 32;

const INFO_KEK: &[u8] = b"havenkeys/v1/kek";
const INFO_KEK_V2: &[u8] = b"havenkeys/v2/kek";
const INFO_DATA: &[u8] = b"havenkeys/v1/data";

/// A 256-bit symmetric key, zeroized on drop.
pub struct Key256(Zeroizing<[u8; KEY_LEN]>);

impl Key256 {
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// Fresh key from the OS CSPRNG.
    pub fn random() -> Result<Self> {
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        fill_random(key.as_mut())?;
        Ok(Self(key))
    }

    /// Only for passing to cipher constructors and for wrapping.
    pub(crate) fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

impl fmt::Debug for Key256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key256(<redacted>)")
    }
}

fn hkdf_expand(ikm: &Key256, salt: Option<&[u8]>, info: &[u8]) -> Result<Key256> {
    let hk = Hkdf::<Sha256>::new(salt, ikm.as_bytes());
    let mut okm = Zeroizing::new([0u8; KEY_LEN]);
    hk.expand(info, okm.as_mut()).map_err(|_| Error::Kdf)?;
    Ok(Key256(okm))
}

/// master key → key-encryption key, bound to this vault's ID.
pub fn derive_kek(master_key: &Key256, vault_id: &Uuid) -> Result<Key256> {
    hkdf_expand(master_key, Some(vault_id.as_bytes()), INFO_KEK)
}

/// master key + Secret Key → key-encryption key (key scheme 2).
///
/// HKDF-SHA-256 with both secrets as input keying material, the vault ID as
/// salt and a v2 label, exactly as planned in docs/crypto.md. Either secret
/// alone gives nothing: the output depends on both.
pub fn derive_kek_with_secret_key(
    master_key: &Key256,
    secret_key: &SecretKey,
    vault_id: &Uuid,
) -> Result<Key256> {
    let mut ikm = Zeroizing::new([0u8; KEY_LEN + SECRET_KEY_LEN]);
    ikm[..KEY_LEN].copy_from_slice(master_key.as_bytes());
    ikm[KEY_LEN..].copy_from_slice(secret_key.as_bytes());
    let hk = Hkdf::<Sha256>::new(Some(vault_id.as_bytes()), ikm.as_ref());
    let mut okm = Zeroizing::new([0u8; KEY_LEN]);
    hk.expand(INFO_KEK_V2, okm.as_mut())
        .map_err(|_| Error::Kdf)?;
    Ok(Key256(okm))
}

/// vault key → data key used for item and settings blobs.
pub fn derive_data_key(vault_key: &Key256) -> Result<Key256> {
    hkdf_expand(vault_key, None, INFO_DATA)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_keys_differ() {
        let a = Key256::random().unwrap();
        let b = Key256::random().unwrap();
        assert_ne!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn kek_is_bound_to_vault_id() {
        let mk = Key256::from_bytes([7u8; 32]);
        let a = derive_kek(&mk, &Uuid::from_u128(1)).unwrap();
        let b = derive_kek(&mk, &Uuid::from_u128(2)).unwrap();
        assert_ne!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn derivations_are_domain_separated() {
        let k = Key256::from_bytes([9u8; 32]);
        let kek = derive_kek(&k, &Uuid::nil()).unwrap();
        let data = derive_data_key(&k).unwrap();
        assert_ne!(kek.as_bytes(), data.as_bytes());
        assert_ne!(data.as_bytes(), k.as_bytes());
    }

    #[test]
    fn debug_is_redacted() {
        let k = Key256::from_bytes([0xAB; 32]);
        assert_eq!(format!("{k:?}"), "Key256(<redacted>)");
    }
}
