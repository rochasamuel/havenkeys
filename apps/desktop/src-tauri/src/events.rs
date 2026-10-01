//! The desktop's answer to `ClientEvents`: Tauri events for the window,
//! the auto-lock clock and the browser bridge.

use crate::state::{AppState, ITEMS_CHANGED_EVENT, LOCKED_EVENT};
use crate::sync::{CONNECTIVITY_EVENT, SIGNED_OUT_EVENT, SYNCED_EVENT};
use havenkeys_client::ClientEvents;
use havenkeys_core::sync::SyncReport;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// This vault reopened empty after "remove this device": the UI returns to
/// the first-run screen. Carries `Removed`.
pub const REMOVED_EVENT: &str = "vault://removed";

pub const KEYCHAIN_NOT_CLEARED: &str = "This computer was removed, but HavenKeys could not delete the Secret Key from the system keychain. Delete the entry \u{201c}app.havenkeys\u{201d} yourself.";

#[derive(Clone, Serialize)]
struct LockedPayload {
    reason: &'static str,
}

/// The `vault://removed` payload.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Removed {
    /// Set when the Secret Key may still be in the system keychain.
    pub keychain_warning: Option<&'static str>,
}

pub struct DesktopEvents {
    pub app: AppHandle,
}

impl ClientEvents for DesktopEvents {
    fn unlocked(&self, auto_lock_minutes: u32) {
        if let Some(state) = self.app.try_state::<AppState>() {
            state.arm_auto_lock(auto_lock_minutes);
            state.notify_unlocked();
        }
    }

    fn locked(&self, reason: &'static str, was_open: bool) {
        if let Some(state) = self.app.try_state::<AppState>() {
            state.clear_after_lock();
            if was_open {
                state.notify_locked();
                let _ = self.app.emit(LOCKED_EVENT, LockedPayload { reason });
            }
        }
    }

    fn connectivity(&self, online: bool) {
        let _ = self.app.emit(CONNECTIVITY_EVENT, online);
    }

    fn signed_out(&self) {
        let _ = self.app.emit(SIGNED_OUT_EVENT, ());
    }

    fn synced(&self, report: SyncReport) {
        let _ = self.app.emit(SYNCED_EVENT, report);
    }

    fn items_changed(&self) {
        let _ = self.app.emit(ITEMS_CHANGED_EVENT, ());
    }

    fn removed(&self, keychain_cleared: bool) {
        let payload = Removed {
            keychain_warning: (!keychain_cleared).then_some(KEYCHAIN_NOT_CLEARED),
        };
        let _ = self.app.emit(REMOVED_EVENT, payload);
    }
}
