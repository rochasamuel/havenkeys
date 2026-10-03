//! Values that belong to this device and never sync: the Android app's own
//! settings, its Digital Asset Links cache, and item activity (uses, recent
//! searches). Sealed with the vault's data
//! key like every other blob, so they read only while unlocked.

use crate::crypto::blob::{BlobContext, Purpose};
use crate::error::Result;
use crate::vault::{open_json, seal_json, VaultService};
use serde::de::DeserializeOwned;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalSlot {
    DeviceSettings,
    AssetLinks,
    Activity,
}

impl LocalSlot {
    fn name(self) -> &'static str {
        match self {
            Self::DeviceSettings => "device_settings",
            Self::AssetLinks => "asset_links",
            Self::Activity => "activity",
        }
    }

    fn purpose(self) -> Purpose {
        match self {
            Self::DeviceSettings => Purpose::DeviceSettings,
            Self::AssetLinks => Purpose::AssetLinks,
            Self::Activity => Purpose::Activity,
        }
    }
}

impl VaultService {
    /// `Ok(None)` when nothing was written yet. A blob that does not open
    /// (another vault's, damaged) is an error; callers fall back to defaults.
    pub fn read_local<T: DeserializeOwned>(&self, slot: LocalSlot) -> Result<Option<T>> {
        let session = self.session()?;
        let Some(blob) = self.store.local_blob(slot.name())? else {
            return Ok(None);
        };
        let ctx = BlobContext::vault(slot.purpose(), session.vault_id);
        open_json(&session.data_key, &ctx, &blob).map(Some)
    }

    pub fn write_local<T: Serialize>(&self, slot: LocalSlot, value: &T) -> Result<()> {
        let session = self.session()?;
        let ctx = BlobContext::vault(slot.purpose(), session.vault_id);
        let blob = seal_json(&session.data_key, &ctx, value)?;
        self.store.set_local_blob(slot.name(), &blob)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::LocalSlot;
    use crate::account::{AccountRef, NormalizedEmail};
    use crate::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
    use crate::store::{AccountRecord, Store};
    use crate::vault::{prepare_new_account_vault, VaultService};
    use crate::{Error, SecretString};
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Prefs {
        flag: bool,
    }

    pub(crate) fn unlocked_vault() -> VaultService {
        let account = AccountRef::new(
            Uuid::from_u128(9),
            NormalizedEmail::parse("user@example.com").unwrap(),
        );
        let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap();
        let made = prepare_new_account_vault(
            &SecretString::from("correct horse battery staple"),
            &account,
            kdf,
            1_700_000_000_000,
        )
        .unwrap();
        let mut vault = VaultService::new(Store::open_in_memory().unwrap());
        let record = AccountRecord {
            account_id: account.id,
            email: "user@example.com".into(),
            server_url: "https://vault.example.com".into(),
            server_cursor: 0,
            max_header_rev: 0,
            last_synced_at: None,
        };
        vault.create_account_vault(made.prepared, &record).unwrap();
        vault
    }

    #[test]
    fn a_slot_reads_back_what_was_written() {
        let vault = unlocked_vault();
        assert_eq!(
            vault
                .read_local::<Prefs>(LocalSlot::DeviceSettings)
                .unwrap(),
            None
        );
        vault
            .write_local(LocalSlot::DeviceSettings, &Prefs { flag: true })
            .unwrap();
        assert_eq!(
            vault
                .read_local::<Prefs>(LocalSlot::DeviceSettings)
                .unwrap(),
            Some(Prefs { flag: true })
        );
        assert_eq!(
            vault.read_local::<Prefs>(LocalSlot::AssetLinks).unwrap(),
            None
        );
    }

    #[test]
    fn a_locked_vault_reads_and_writes_nothing() {
        let mut vault = unlocked_vault();
        vault.lock();
        assert_eq!(
            vault
                .read_local::<Prefs>(LocalSlot::DeviceSettings)
                .unwrap_err(),
            Error::Locked
        );
        assert_eq!(
            vault
                .write_local(LocalSlot::DeviceSettings, &Prefs { flag: true })
                .unwrap_err(),
            Error::Locked
        );
    }

    #[test]
    fn a_slot_does_not_open_in_another_vault_or_under_another_purpose() {
        let a = unlocked_vault();
        let b = unlocked_vault();
        a.write_local(LocalSlot::DeviceSettings, &Prefs { flag: true })
            .unwrap();
        let blob = a.store.local_blob("device_settings").unwrap().unwrap();
        b.store.set_local_blob("device_settings", &blob).unwrap();
        assert!(b.read_local::<Prefs>(LocalSlot::DeviceSettings).is_err());
        a.store.set_local_blob("asset_links", &blob).unwrap();
        assert!(a.read_local::<Prefs>(LocalSlot::AssetLinks).is_err());
    }

    #[test]
    fn the_activity_slot_is_its_own() {
        let vault = unlocked_vault();
        vault
            .write_local(LocalSlot::Activity, &Prefs { flag: true })
            .unwrap();
        assert_eq!(
            vault.read_local::<Prefs>(LocalSlot::Activity).unwrap(),
            Some(Prefs { flag: true })
        );
        assert_eq!(
            vault
                .read_local::<Prefs>(LocalSlot::DeviceSettings)
                .unwrap(),
            None
        );
        // Sealed under its own purpose: another slot's blob does not open here.
        let blob = vault.store.local_blob("activity").unwrap().unwrap();
        vault
            .store
            .set_local_blob("device_settings", &blob)
            .unwrap();
        assert!(vault
            .read_local::<Prefs>(LocalSlot::DeviceSettings)
            .is_err());
    }
}
