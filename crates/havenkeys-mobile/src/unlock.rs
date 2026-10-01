use crate::error::MobileResult;
use crate::vault::{MobileVault, Status};
use havenkeys_core::{SecretBytes, SecretString};

#[uniffi::export]
impl MobileVault {
    pub fn unlock_password(&self, password: String) -> MobileResult<Status> {
        let client = self.client.clone();
        self.block_on(client.unlock(SecretString::new(password), None))?;
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
    use crate::vault::tests::{unlocked, wait_for, PASSWORD};
    use crate::vault::LockState;

    #[test]
    fn the_master_password_unlocks() {
        let dir = tempfile::tempdir().unwrap();
        let (vault, _) = unlocked(dir.path());
        vault.lock();
        assert!(vault.unlock_password("wrong password!".into()).is_err());
        let s = vault.unlock_password(PASSWORD.into()).unwrap();
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
}
