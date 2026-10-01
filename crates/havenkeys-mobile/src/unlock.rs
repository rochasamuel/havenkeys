use crate::error::MobileResult;
use crate::vault::{MobileVault, Status};
use havenkeys_core::{SecretBytes, SecretString};

#[uniffi::export]
impl MobileVault {
    /// `secret_key` is the Emergency Kit's key, typed when this device lacks
    /// it (`Status::needs_secret_key`); the device keeps it after unlocking.
    pub fn unlock_password(
        &self,
        password: String,
        secret_key: Option<String>,
    ) -> MobileResult<Status> {
        let client = self.client.clone();
        self.block_on(client.unlock(
            SecretString::new(password),
            secret_key.map(SecretString::new),
        ))?;
        self.status()
    }

    /// The bytes the app seals with its biometric Keystore key. The app
    /// zeroes its copy right after sealing.
    pub fn create_unlock_bundle(&self, password: String, boot_count: i64) -> MobileResult<Vec<u8>> {
        let client = self.client.clone();
        let bundle =
            self.block_on(client.create_unlock_bundle(SecretString::new(password), boot_count))?;
        Ok(bundle.expose().to_vec())
    }

    pub fn unlock_with_bundle(&self, bundle: Vec<u8>, boot_count: i64) -> MobileResult<Status> {
        let bundle = SecretBytes::new(bundle);
        let client = self.client.clone();
        self.block_on(client.unlock_with_bundle(bundle, boot_count))?;
        self.status()
    }
}

#[cfg(test)]
mod tests {
    use crate::vault::tests::{mobile, unlocked, wait_for, PASSWORD};
    use crate::vault::LockState;

    #[test]
    fn the_master_password_unlocks() {
        let dir = tempfile::tempdir().unwrap();
        let (vault, _) = unlocked(dir.path());
        vault.lock();
        assert!(vault
            .unlock_password("wrong password!".into(), None)
            .is_err());
        let s = vault.unlock_password(PASSWORD.into(), None).unwrap();
        assert!(matches!(s.state, LockState::Unlocked));
    }

    #[test]
    fn a_bundle_unlocks_offline_and_a_stale_one_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let (vault, seen) = unlocked(dir.path());
        let bundle = vault.create_unlock_bundle(PASSWORD.into(), 3).unwrap();
        vault.lock();
        wait_for(|| seen.has("locked:user"));
        assert_eq!(
            match vault.unlock_with_bundle(bundle.clone(), 4).err().unwrap() {
                crate::MobileError::Failed { code, .. } => code,
            },
            "bundle_refused"
        );
        let s = vault.unlock_with_bundle(bundle, 3).unwrap();
        assert!(matches!(s.state, LockState::Unlocked));
        // Offline (the seeded server does not exist): it stays unlocked.
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(matches!(vault.status().unwrap().state, LockState::Unlocked));
    }

    #[test]
    fn enrolling_while_locked_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let (vault, _) = unlocked(dir.path());
        vault.lock();
        assert!(vault.create_unlock_bundle(PASSWORD.into(), 1).is_err());
    }

    fn code(r: crate::MobileResult<crate::vault::Status>) -> String {
        match r.err().unwrap() {
            crate::MobileError::Failed { code, .. } => code,
        }
    }

    #[test]
    fn a_device_without_the_secret_key_unlocks_with_the_typed_one() {
        let dir = tempfile::tempdir().unwrap();
        let (vault, _) = mobile(dir.path());
        let key = havenkeys_client::testing::seed_account_vault(&vault.client, PASSWORD);
        vault.lock();
        vault
            .client
            .device()
            .unwrap()
            .forget(havenkeys_client::testing::ACCOUNT);
        assert!(vault.status().unwrap().needs_secret_key);

        assert_eq!(
            code(vault.unlock_password(PASSWORD.into(), None)),
            "secret_key_required"
        );
        let other = havenkeys_core::crypto::secret_key::SecretKey::generate().unwrap();
        assert!(vault
            .unlock_password(PASSWORD.into(), Some(other.to_text().expose().to_owned()))
            .is_err());
        assert!(matches!(vault.status().unwrap().state, LockState::Locked));

        let s = vault
            .unlock_password(PASSWORD.into(), Some(key.to_text().expose().to_owned()))
            .unwrap();
        assert!(matches!(s.state, LockState::Unlocked));
        assert!(!s.needs_secret_key);
    }
}
