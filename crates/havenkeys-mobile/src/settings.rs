use crate::error::MobileResult;
use crate::vault::MobileVault;
use havenkeys_core::local::LocalSlot;
use serde::{Deserialize, Serialize};

/// This phone's own settings: never synced, sealed in a device-local slot.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct DeviceSettings {
    pub lock_on_screen_off: bool,
    pub confirm_before_filling: bool,
    pub asset_links: bool,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            lock_on_screen_off: true,
            confirm_before_filling: false,
            asset_links: true,
        }
    }
}

impl DeviceSettings {
    /// Nothing written yet: the defaults. A blob that is there but does not
    /// open (damaged, replaced) or a locked vault: the defaults with
    /// "Confirm before filling" on, so a bad blob never turns it off.
    fn from_slot(read: havenkeys_core::Result<Option<Self>>) -> Self {
        match read {
            Ok(Some(settings)) => settings,
            Ok(None) => Self::default(),
            Err(_) => Self {
                confirm_before_filling: true,
                ..Self::default()
            },
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct MobileSettings {
    pub auto_lock_minutes: u32,
    pub clipboard_clear_seconds: u32,
    pub lock_on_screen_off: bool,
    pub confirm_before_filling: bool,
    pub asset_links: bool,
}

impl MobileVault {
    pub(crate) fn device_settings(&self) -> DeviceSettings {
        DeviceSettings::from_slot(match self.client.vault() {
            Ok(v) => v.read_local(LocalSlot::DeviceSettings),
            Err(_) => Err(havenkeys_core::Error::Locked),
        })
    }
}

#[uniffi::export]
impl MobileVault {
    pub fn settings(&self) -> MobileResult<MobileSettings> {
        let vault = self.client.vault()?;
        let shared = vault.settings()?;
        let device = DeviceSettings::from_slot(vault.read_local(LocalSlot::DeviceSettings));
        Ok(MobileSettings {
            auto_lock_minutes: shared.auto_lock_minutes,
            clipboard_clear_seconds: shared.clipboard_clear_seconds,
            lock_on_screen_off: device.lock_on_screen_off,
            confirm_before_filling: device.confirm_before_filling,
            asset_links: device.asset_links,
        })
    }

    pub fn update_settings(&self, s: MobileSettings) -> MobileResult<()> {
        let mut vault = self.client.vault()?;
        let mut shared = vault.settings()?;
        shared.auto_lock_minutes = s.auto_lock_minutes;
        shared.clipboard_clear_seconds = s.clipboard_clear_seconds;
        vault.update_settings(shared)?;
        vault.write_local(
            LocalSlot::DeviceSettings,
            &DeviceSettings {
                lock_on_screen_off: s.lock_on_screen_off,
                confirm_before_filling: s.confirm_before_filling,
                asset_links: s.asset_links,
            },
        )?;
        drop(vault);
        self.clock.arm(s.auto_lock_minutes);
        Ok(())
    }
}
