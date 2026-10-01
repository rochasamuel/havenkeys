//! The biometric unlock bundle (spec 2026-10-01-android-app §5.2): the vault
//! key and the auth key, with when and in which boot they were enrolled. The
//! app encrypts it with a biometric-bound Android Keystore key; this module
//! builds, parses and judges it.
//!
//! Layout, 81 bytes: version (1) ‖ vault key (32) ‖ auth key (32) ‖
//! enrolled_at_ms (i64, big-endian) ‖ boot_count (i64, big-endian).

use crate::crypto::keys::{AuthKey, Key256, KEY_LEN};
use crate::error::{Error, Result};
use crate::secret::SecretBytes;
use std::fmt;
use zeroize::Zeroizing;

pub const BUNDLE_VERSION: u8 = 1;
pub const BUNDLE_LEN: usize = 1 + KEY_LEN + KEY_LEN + 8 + 8;
/// 14 days: past this the master password is asked for again.
pub const BUNDLE_MAX_AGE_MS: i64 = 14 * 24 * 60 * 60 * 1000;

pub struct UnlockBundle {
    vault_key: Key256,
    auth_key: AuthKey,
    enrolled_at_ms: i64,
    boot_count: i64,
}

impl fmt::Debug for UnlockBundle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UnlockBundle(<redacted>)")
    }
}

impl UnlockBundle {
    pub(crate) fn new(
        vault_key: Key256,
        auth_key: AuthKey,
        enrolled_at_ms: i64,
        boot_count: i64,
    ) -> Self {
        Self {
            vault_key,
            auth_key,
            enrolled_at_ms,
            boot_count,
        }
    }

    pub fn encode(&self) -> SecretBytes {
        let mut out = Zeroizing::new(Vec::with_capacity(BUNDLE_LEN));
        out.push(BUNDLE_VERSION);
        out.extend_from_slice(self.vault_key.as_bytes());
        out.extend_from_slice(self.auth_key.as_bytes());
        out.extend_from_slice(&self.enrolled_at_ms.to_be_bytes());
        out.extend_from_slice(&self.boot_count.to_be_bytes());
        SecretBytes::new(std::mem::take(&mut *out))
    }

    /// Anything but a well-formed version-1 bundle is refused. The bytes
    /// come from Keystore-authenticated decryption, so a refusal here means
    /// a bug or a format change, never a guess to be retried.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != BUNDLE_LEN || bytes[0] != BUNDLE_VERSION {
            return Err(Error::BundleRefused);
        }
        let mut vault_key = Zeroizing::new([0u8; KEY_LEN]);
        vault_key.copy_from_slice(&bytes[1..1 + KEY_LEN]);
        let mut auth_key = Zeroizing::new([0u8; KEY_LEN]);
        auth_key.copy_from_slice(&bytes[1 + KEY_LEN..1 + 2 * KEY_LEN]);
        let tail = &bytes[1 + 2 * KEY_LEN..];
        let enrolled_at_ms =
            i64::from_be_bytes(tail[..8].try_into().map_err(|_| Error::BundleRefused)?);
        let boot_count =
            i64::from_be_bytes(tail[8..16].try_into().map_err(|_| Error::BundleRefused)?);
        Ok(Self {
            vault_key: Key256::from_bytes(*vault_key),
            auth_key: AuthKey::from_bytes(*auth_key),
            enrolled_at_ms,
            boot_count,
        })
    }

    /// Refuses a bundle older than 14 days, one dated in the future (a clock
    /// set back must not stretch the window), and one from another boot. An
    /// unknown boot count (negative) on either side matches no boot.
    pub fn check_fresh(&self, now_ms: i64, boot_count: i64) -> Result<()> {
        let age = now_ms.saturating_sub(self.enrolled_at_ms);
        if !(0..=BUNDLE_MAX_AGE_MS).contains(&age)
            || boot_count < 0
            || boot_count != self.boot_count
        {
            return Err(Error::BundleRefused);
        }
        Ok(())
    }

    pub fn into_keys(self) -> (Key256, AuthKey) {
        (self.vault_key, self.auth_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{AccountRef, NormalizedEmail};
    use crate::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
    use crate::crypto::secret_key::SecretKey;
    use crate::store::{AccountRecord, Store};
    use crate::vault::{prepare_new_account_vault, VaultService, VaultState};
    use crate::SecretString;
    use uuid::Uuid;

    const PASSWORD: &str = "correct horse battery staple";
    const NOW: i64 = 1_800_000_000_000;

    fn account() -> AccountRef {
        AccountRef::new(
            Uuid::from_u128(3),
            NormalizedEmail::parse("user@example.com").unwrap(),
        )
    }

    fn vault() -> (VaultService, SecretKey) {
        let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap();
        let made =
            prepare_new_account_vault(&SecretString::from(PASSWORD), &account(), kdf, NOW).unwrap();
        let mut v = VaultService::new(Store::open_in_memory().unwrap());
        let record = AccountRecord {
            account_id: account().id,
            email: "user@example.com".into(),
            server_url: "https://vault.example.com".into(),
            server_cursor: 0,
            max_header_rev: 0,
            last_synced_at: None,
        };
        v.create_account_vault(made.prepared, &record).unwrap();
        (v, made.secret_key)
    }

    fn enrolled(v: &VaultService, sk: &SecretKey, boot: i64) -> UnlockBundle {
        v.begin_bundle()
            .unwrap()
            .derive(&SecretString::from(PASSWORD), sk, &account(), NOW, boot)
            .unwrap()
    }

    #[test]
    fn a_bundle_round_trips_and_unlocks_its_vault() {
        let (mut v, sk) = vault();
        let bytes = enrolled(&v, &sk, 7).encode();
        assert_eq!(bytes.expose().len(), BUNDLE_LEN);
        v.lock();
        let bundle = UnlockBundle::decode(bytes.expose()).unwrap();
        bundle.check_fresh(NOW + 1_000, 7).unwrap();
        let (vault_key, _auth) = bundle.into_keys();
        v.unlock_with_vault_key(&vault_key).unwrap();
        assert_eq!(v.state(), VaultState::Unlocked);
    }

    #[test]
    fn a_wrong_master_password_enrolls_nothing() {
        let (v, sk) = vault();
        let err = v
            .begin_bundle()
            .unwrap()
            .derive(
                &SecretString::from("wrong password!!"),
                &sk,
                &account(),
                NOW,
                1,
            )
            .unwrap_err();
        assert_eq!(err, Error::UnlockFailed);
    }

    #[test]
    fn enrolling_needs_an_unlocked_vault() {
        let (mut v, _) = vault();
        v.lock();
        assert_eq!(v.begin_bundle().err(), Some(Error::Locked));
    }

    #[test]
    fn an_old_a_future_or_another_boots_bundle_is_refused() {
        let (v, sk) = vault();
        let b = enrolled(&v, &sk, 7);
        assert!(b.check_fresh(NOW + BUNDLE_MAX_AGE_MS, 7).is_ok());
        assert_eq!(
            b.check_fresh(NOW + BUNDLE_MAX_AGE_MS + 1, 7),
            Err(Error::BundleRefused)
        );
        assert_eq!(b.check_fresh(NOW - 1, 7), Err(Error::BundleRefused));
        assert_eq!(b.check_fresh(NOW, 8), Err(Error::BundleRefused));
    }

    #[test]
    fn an_unknown_boot_count_is_refused() {
        let (v, sk) = vault();
        let err = v
            .begin_bundle()
            .unwrap()
            .derive(&SecretString::from(PASSWORD), &sk, &account(), NOW, -1)
            .unwrap_err();
        assert_eq!(err, Error::BundleRefused);
        // A stored -1 (from a bundle built before this check) matches no boot.
        let (vault_key, auth_key) = enrolled(&v, &sk, 0).into_keys();
        let stored = UnlockBundle::new(vault_key, auth_key, NOW, -1);
        assert_eq!(stored.check_fresh(NOW, -1), Err(Error::BundleRefused));
        assert_eq!(
            enrolled(&v, &sk, 7).check_fresh(NOW, -1),
            Err(Error::BundleRefused)
        );
        assert!(enrolled(&v, &sk, 0).check_fresh(NOW, 0).is_ok());
    }

    #[test]
    fn a_tampered_or_truncated_bundle_is_refused() {
        let (v, sk) = vault();
        let bytes = enrolled(&v, &sk, 7).encode();
        let good = bytes.expose().to_vec();
        assert!(UnlockBundle::decode(&good[..BUNDLE_LEN - 1]).is_err());
        let mut longer = good.clone();
        longer.push(0);
        assert!(UnlockBundle::decode(&longer).is_err());
        let mut version = good.clone();
        version[0] = 2;
        assert_eq!(
            UnlockBundle::decode(&version).err(),
            Some(Error::BundleRefused)
        );
    }

    #[test]
    fn a_random_key_or_another_vaults_key_does_not_unlock() {
        let (mut v, _) = vault();
        let (other, other_sk) = vault();
        v.lock();
        let random = Key256::random().unwrap();
        assert_eq!(v.unlock_with_vault_key(&random), Err(Error::BundleRefused));
        assert_eq!(v.state(), VaultState::Locked);
        let (other_key, _) = enrolled(&other, &other_sk, 1).into_keys();
        assert_eq!(
            v.unlock_with_vault_key(&other_key),
            Err(Error::BundleRefused)
        );
        assert_eq!(v.state(), VaultState::Locked);
    }

    #[test]
    fn an_unlocked_vault_is_not_unlocked_again() {
        let (mut v, sk) = vault();
        let (key, _) = enrolled(&v, &sk, 1).into_keys();
        assert!(v.unlock_with_vault_key(&key).is_err());
    }

    #[test]
    fn a_bundle_prints_nothing() {
        let (v, sk) = vault();
        assert_eq!(
            format!("{:?}", enrolled(&v, &sk, 1)),
            "UnlockBundle(<redacted>)"
        );
    }
}
