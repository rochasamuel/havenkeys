//! 256-bit key material and HKDF key separation.

use crate::account::AccountRef;
use crate::crypto::fill_random;
use crate::crypto::secret_key::{SecretKey, SECRET_KEY_LEN};
use crate::error::{Error, Result};
use data_encoding::BASE64;
use hkdf::Hkdf;
use sha2::Sha256;
use std::fmt;
use zeroize::Zeroizing;

pub const KEY_LEN: usize = 32;

const INFO_DATA: &[u8] = b"havenkeys/v1/data";
const INFO_KEK_V3: &[u8] = b"havenkeys/v3/kek";
const INFO_AUTH_V3: &[u8] = b"havenkeys/v3/auth";
const INFO_IDENTITY_ID: &[u8] = b"havenkeys/v3/identity-item-id";

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

/// HKDF-SHA256 to one 256-bit output, zeroized on drop.
fn hkdf_sha256(salt: Option<&[u8]>, ikm: &[u8], info: &[u8]) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    let hk = Hkdf::<Sha256>::new(salt, ikm);
    let mut okm = Zeroizing::new([0u8; KEY_LEN]);
    hk.expand(info, okm.as_mut()).map_err(|_| Error::Kdf)?;
    Ok(okm)
}

/// Proof that the holder knows the master password and the Secret Key, sent
/// to the sync server at login.
///
/// It authenticates and nothing else: it unwraps no key, and HKDF's `info`
/// separation means it cannot be walked back to the KEK, the master key or
/// the Secret Key. Never log it, never persist it.
pub struct AuthKey(Zeroizing<[u8; KEY_LEN]>);

impl AuthKey {
    /// For the wire. Standard Base64, padded.
    pub fn to_base64(&self) -> Zeroizing<String> {
        Zeroizing::new(BASE64.encode(self.0.as_ref()))
    }

    /// Only the unlock bundle rebuilds an auth key from bytes.
    pub(crate) fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    pub(crate) fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

impl fmt::Debug for AuthKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AuthKey(<redacted>)")
    }
}

/// HKDF salt for key scheme 3: the account UUID's 16 bytes, then the
/// normalized email. Both devices must build this identically or they derive
/// different keys, which is why the email is normalized at parse time.
fn account_salt(account: &AccountRef) -> Zeroizing<Vec<u8>> {
    let email = account.email.as_str().as_bytes();
    let mut salt = Zeroizing::new(Vec::with_capacity(16 + email.len()));
    salt.extend_from_slice(account.id.as_bytes());
    salt.extend_from_slice(email);
    salt
}

/// One HKDF expansion over (master key ‖ Secret Key), bound to the account.
fn account_bound(
    master_key: &Key256,
    secret_key: &SecretKey,
    account: &AccountRef,
    info: &[u8],
) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    let mut ikm = Zeroizing::new([0u8; KEY_LEN + SECRET_KEY_LEN]);
    ikm[..KEY_LEN].copy_from_slice(master_key.as_bytes());
    ikm[KEY_LEN..].copy_from_slice(secret_key.as_bytes());
    hkdf_sha256(Some(account_salt(account).as_ref()), ikm.as_ref(), info)
}

/// master key + Secret Key → key-encryption key, bound to the account
/// (key scheme 3). See docs/crypto.md.
pub fn derive_kek_v3(
    master_key: &Key256,
    secret_key: &SecretKey,
    account: &AccountRef,
) -> Result<Key256> {
    account_bound(master_key, secret_key, account, INFO_KEK_V3).map(Key256)
}

/// master key + Secret Key → the server auth key (key scheme 3). Same input
/// keying material as the KEK, separated by the HKDF label.
pub fn derive_auth_key_from_master(
    master_key: &Key256,
    secret_key: &SecretKey,
    account: &AccountRef,
) -> Result<AuthKey> {
    account_bound(master_key, secret_key, account, INFO_AUTH_V3).map(AuthKey)
}

/// vault key → data key used for item and settings blobs.
pub fn derive_data_key(vault_key: &Key256) -> Result<Key256> {
    hkdf_sha256(None, vault_key.as_bytes(), INFO_DATA).map(Key256)
}

/// vault key → the item ID of the account's one Identity.
///
/// Every device holding the vault key computes the same ID, so the identity
/// is created once however many devices race to create it; the server, which
/// never has the vault key, cannot tell which item it is
/// (spec 2026-09-29-identity-item §5.1).
pub fn derive_identity_item_id(vault_key: &Key256) -> Result<uuid::Uuid> {
    let hk = Hkdf::<Sha256>::new(None, vault_key.as_bytes());
    let mut okm = [0u8; 16];
    hk.expand(INFO_IDENTITY_ID, &mut okm)
        .map_err(|_| Error::Kdf)?;
    Ok(uuid::Uuid::new_v8(okm))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn identity_item_id_matches_the_independent_vector() {
        // HKDF-SHA256(ikm = 0x09 * 32, no salt, info), first 16 bytes, v8
        // bits set; computed with Python's hmac module.
        let id = derive_identity_item_id(&Key256::from_bytes([9u8; 32])).unwrap();
        assert_eq!(id.to_string(), "00f03a59-33cc-8082-a649-ca545b3b372d");
        assert_eq!(id.get_version_num(), 8);
        let other = derive_identity_item_id(&Key256::from_bytes([10u8; 32])).unwrap();
        assert_ne!(id, other);
    }

    #[test]
    fn random_keys_differ() {
        let a = Key256::random().unwrap();
        let b = Key256::random().unwrap();
        assert_ne!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn debug_is_redacted() {
        let k = Key256::from_bytes([0xAB; 32]);
        assert_eq!(format!("{k:?}"), "Key256(<redacted>)");
    }

    use crate::account::{AccountRef, NormalizedEmail};

    fn test_account() -> AccountRef {
        AccountRef::new(
            Uuid::parse_str("00112233-4455-6677-8899-aabbccddeeff").unwrap(),
            NormalizedEmail::parse("user@example.com").unwrap(),
        )
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn v3_derivation_matches_the_independent_vector() {
        let mk = Key256::from_bytes([0x11; 32]);
        let sk = SecretKey::from_bytes([0x22; SECRET_KEY_LEN]);
        let kek = derive_kek_v3(&mk, &sk, &test_account()).unwrap();
        let auth = derive_auth_key_from_master(&mk, &sk, &test_account()).unwrap();
        assert_eq!(
            hex(kek.as_bytes()),
            "4f13abed658c5db4ad2a62698c1cf1e80491be5643591dc00dc88947ea7506e9"
        );
        assert_eq!(
            hex(auth.as_bytes()),
            "b586a25c57588b9b1cea48ea057e622edf4d5bcfc366e8ebf7738c1f78ce91b0"
        );
    }

    #[test]
    fn data_key_matches_the_independent_vector() {
        // HKDF-SHA256(ikm = 0x09 * 32, no salt, info = "havenkeys/v1/data"),
        // 32 bytes; computed with Python's hmac module.
        let data = derive_data_key(&Key256::from_bytes([9u8; 32])).unwrap();
        assert_eq!(
            hex(data.as_bytes()),
            "30ab54695a48584d577a00260aacaf995674778a1fb588b4b7745656f1d6d234"
        );
    }

    /// `derive_data_key` must not be a disguised identity function, and its
    /// `INFO_DATA` label must not collide with the KEK's label: if it ever
    /// did, `seal`/`open` would still round-trip and every other test would
    /// still pass, while the vault key itself did the sealing instead of a
    /// key separated from it. Restores the coverage
    /// `derivations_are_domain_separated` had against the removed v1
    /// `derive_kek`, now against the derivations that still exist:
    /// `derive_kek_v3` in place of `derive_kek`, same `derive_data_key`.
    #[test]
    fn data_key_differs_from_the_vault_key_and_the_kek() {
        let vault_key = Key256::from_bytes([9u8; 32]);
        let sk = SecretKey::from_bytes([0x22; SECRET_KEY_LEN]);
        let data = derive_data_key(&vault_key).unwrap();
        let kek = derive_kek_v3(&vault_key, &sk, &test_account()).unwrap();
        assert_ne!(data.as_bytes(), kek.as_bytes());
        assert_ne!(data.as_bytes(), vault_key.as_bytes());
    }

    #[test]
    fn kek_and_auth_key_are_domain_separated() {
        let mk = Key256::from_bytes([0x11; 32]);
        let sk = SecretKey::from_bytes([0x22; SECRET_KEY_LEN]);
        let kek = derive_kek_v3(&mk, &sk, &test_account()).unwrap();
        let auth = derive_auth_key_from_master(&mk, &sk, &test_account()).unwrap();
        assert_ne!(kek.as_bytes(), auth.as_bytes());
    }

    #[test]
    fn v3_is_bound_to_the_account_id_and_the_email() {
        let mk = Key256::from_bytes([0x11; 32]);
        let sk = SecretKey::from_bytes([0x22; SECRET_KEY_LEN]);
        let base = derive_kek_v3(&mk, &sk, &test_account()).unwrap();

        let other_id = AccountRef::new(
            Uuid::from_u128(1),
            NormalizedEmail::parse("user@example.com").unwrap(),
        );
        let other_email = AccountRef::new(
            Uuid::parse_str("00112233-4455-6677-8899-aabbccddeeff").unwrap(),
            NormalizedEmail::parse("other@example.com").unwrap(),
        );
        assert_ne!(
            base.as_bytes(),
            derive_kek_v3(&mk, &sk, &other_id).unwrap().as_bytes()
        );
        assert_ne!(
            base.as_bytes(),
            derive_kek_v3(&mk, &sk, &other_email).unwrap().as_bytes()
        );
    }

    #[test]
    fn v3_needs_both_secrets() {
        let account = test_account();
        let mk_a = Key256::from_bytes([0x11; 32]);
        let mk_b = Key256::from_bytes([0x12; 32]);
        let sk_a = SecretKey::from_bytes([0x22; SECRET_KEY_LEN]);
        let sk_b = SecretKey::from_bytes([0x23; SECRET_KEY_LEN]);
        let base = derive_kek_v3(&mk_a, &sk_a, &account).unwrap();
        assert_ne!(
            base.as_bytes(),
            derive_kek_v3(&mk_b, &sk_a, &account).unwrap().as_bytes()
        );
        assert_ne!(
            base.as_bytes(),
            derive_kek_v3(&mk_a, &sk_b, &account).unwrap().as_bytes()
        );
    }

    #[test]
    fn auth_key_debug_is_redacted() {
        let mk = Key256::from_bytes([0x11; 32]);
        let sk = SecretKey::from_bytes([0x22; SECRET_KEY_LEN]);
        let auth = derive_auth_key_from_master(&mk, &sk, &test_account()).unwrap();
        assert_eq!(format!("{auth:?}"), "AuthKey(<redacted>)");
    }
}
