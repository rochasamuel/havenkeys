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

#[derive(Clone, uniffi::Record)]
pub struct MobileSettings {
    pub auto_lock_minutes: u32,
    pub clipboard_clear_seconds: u32,
    pub lock_on_screen_off: bool,
    pub confirm_before_filling: bool,
    pub asset_links: bool,
}

impl MobileVault {
    /// Defaults when locked or unreadable: the safe ones (screen-off lock
    /// on, Digital Asset Links on, confirm off as the spec's default).
    pub(crate) fn device_settings(&self) -> DeviceSettings {
        self.client
            .vault()
            .ok()
            .and_then(|v| {
                v.read_local::<DeviceSettings>(LocalSlot::DeviceSettings)
                    .ok()
                    .flatten()
            })
            .unwrap_or_default()
    }
}

#[uniffi::export]
impl MobileVault {
    pub fn settings(&self) -> MobileResult<MobileSettings> {
        let vault = self.client.vault()?;
        let shared = vault.settings()?;
        let device = vault
            .read_local::<DeviceSettings>(LocalSlot::DeviceSettings)
            .ok()
            .flatten()
            .unwrap_or_default();
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
