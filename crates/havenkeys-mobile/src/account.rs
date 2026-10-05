use crate::error::MobileResult;
use crate::items::parse_id;
use crate::vault::MobileVault;

#[derive(uniffi::Record)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub last_seen_at: Option<String>,
    pub current: bool,
    pub approved_by: Option<String>,
}

#[uniffi::export]
impl MobileVault {
    pub fn sync_now(&self) -> MobileResult<()> {
        let client = self.client.clone();
        self.block_on(client.sync_now())?;
        Ok(())
    }

    /// The periodic pull while unlocked and in the foreground; while the
    /// server is unreachable it is also how the device finds it again.
    pub fn sync_if_due(&self) -> MobileResult<()> {
        if self.client.pull_due() {
            self.sync_now()?;
        }
        Ok(())
    }

    pub fn devices(&self) -> MobileResult<Vec<DeviceInfo>> {
        let client = self.client.clone();
        Ok(self
            .block_on(client.list_devices())?
            .into_iter()
            .map(|d| DeviceInfo {
                id: d.id.to_string(),
                name: d.name,
                created_at: d.created_at,
                last_seen_at: d.last_seen_at,
                current: d.current,
                approved_by: d.approved_by.map(|u| u.to_string()),
            })
            .collect())
    }

    pub fn revoke_device(&self, id: String) -> MobileResult<()> {
        let id = parse_id(&id)?;
        let client = self.client.clone();
        self.block_on(client.revoke_device(id))?;
        self.revoked(id);
        Ok(())
    }

    /// Ends the session and locks. The app also deletes its biometric key
    /// and bundle (spec §6.4).
    pub fn sign_out(&self) -> MobileResult<()> {
        let client = self.client.clone();
        Ok(self.block_on(client.sign_out())?)
    }

    /// Sets the vault file aside and forgets the Secret Key file; the app
    /// deletes its Keystore keys and bundle on the `removed` event.
    pub fn remove_device(&self, confirmation: String) -> MobileResult<()> {
        let client = self.client.clone();
        Ok(self.block_on(client.remove_device(confirmation))?)
    }

    /// Deletes the account on the server, then this phone's copy (vault
    /// file and Secret Key file); the app deletes its Keystore keys on the
    /// `account_deleted` event.
    pub fn delete_account(
        &self,
        confirmation: String,
        master_password: String,
    ) -> MobileResult<()> {
        let client = self.client.clone();
        Ok(self.block_on(client.delete_account(
            confirmation,
            havenkeys_core::SecretString::new(master_password),
        ))?)
    }
}

impl MobileVault {
    /// Revoking this phone ends its access as signing out does: on
    /// `signed_out` the app deletes its biometric key and unlock bundle, so
    /// the bundle cannot keep opening the replica offline (security-review
    /// AN8).
    fn revoked(&self, id: uuid::Uuid) {
        if self.client.device_id().is_ok_and(|own| own == id) {
            havenkeys_client::ClientEvents::signed_out(&*self.events);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::vault::tests::unlocked;
    use crate::vault::LockState;

    #[test]
    fn an_offline_device_syncs_nothing_and_cannot_list_or_revoke() {
        let dir = tempfile::tempdir().unwrap();
        let (v, _) = unlocked(dir.path());
        v.sync_if_due().unwrap();
        assert!(v.devices().is_err());
        assert!(v.revoke_device("not-an-id".into()).is_err());
    }

    #[test]
    fn revoking_this_phone_signs_it_out_and_another_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let (v, seen) = unlocked(dir.path());
        v.revoked(uuid::Uuid::new_v4());
        v.revoked(v.client.device_id().unwrap());
        // Events arrive in order: by the time this phone's arrives, another
        // device's revocation would already have been seen.
        crate::vault::tests::wait_for(|| seen.has("signed_out"));
        assert_eq!(
            seen.0
                .lock()
                .unwrap()
                .iter()
                .filter(|e| *e == "signed_out")
                .count(),
            1
        );
    }

    #[test]
    fn signing_out_locks_and_a_wrong_confirmation_removes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (v, seen) = unlocked(dir.path());
        assert!(v.remove_device("wrong".into()).is_err());
        assert!(matches!(v.status().unwrap().state, LockState::Unlocked));
        v.sign_out().unwrap();
        assert!(matches!(v.status().unwrap().state, LockState::Locked));
        assert!(seen.has("locked:user"));
    }
}
