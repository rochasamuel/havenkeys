//! The desktop's answer to `ClientEvents`: Tauri events for the window,
//! the auto-lock clock and the browser bridge.

use crate::state::{AppState, ITEMS_CHANGED_EVENT, LOCKED_EVENT};
use crate::sync::{CONNECTIVITY_EVENT, SIGNED_OUT_EVENT, SYNCED_EVENT};
use havenkeys_client::ClientEvents;
use havenkeys_core::sync::SyncReport;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// The account's plan changed (trial ended, subscribed, payment lapsed): the
/// UI reloads the account status. Carries nothing.
pub const PLAN_CHANGED_EVENT: &str = "plan_changed";

/// This vault reopened empty after "remove this device": the UI returns to
/// the first-run screen. Carries `Removed`.
pub const REMOVED_EVENT: &str = "vault://removed";

/// The account was deleted (here or on another device) and this computer's
/// copy erased: the UI returns to first run and says why. Carries `Removed`.
pub const ACCOUNT_DELETED_EVENT: &str = "vault://account-deleted";

const KEYCHAIN_NOT_CLEARED_DELETED: &str = "The account was deleted, but HavenKeys could not delete the Secret Key from the system keychain. Delete the entry \u{201c}app.havenkeys\u{201d} yourself.";

const KEYCHAIN_NOT_CLEARED: &str = "This computer was removed, but HavenKeys could not delete the Secret Key from the system keychain. Delete the entry \u{201c}app.havenkeys\u{201d} yourself.";

#[derive(Clone, Serialize)]
struct LockedPayload {
    reason: &'static str,
}

/// The `vault://removed` payload.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Removed {
    /// Set when the Secret Key may still be in the system keychain.
    keychain_warning: Option<&'static str>,
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

    fn plan_changed(&self) {
        let _ = self.app.emit(PLAN_CHANGED_EVENT, ());
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

    fn account_deleted(&self, keychain_cleared: bool) {
        let payload = Removed {
            keychain_warning: (!keychain_cleared).then_some(KEYCHAIN_NOT_CLEARED_DELETED),
        };
        let _ = self.app.emit(ACCOUNT_DELETED_EVENT, payload);
    }
}
