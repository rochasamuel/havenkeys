//! Shared application state.
//!
//! Lock ordering (to avoid deadlocks): `vault` before `lock_manager`, and
//! `vault` before the bridge's connection list (`notify` may run under the
//! vault lock; the bridge never takes the vault while holding its own locks).

use crate::clipboard::ClipboardGuard;
use crate::device::Device;
use havenkeys_bridge::Bridge;
use havenkeys_core::lock::LockManager;
use havenkeys_core::vault::VaultService;
use havenkeys_protocol::Event;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

pub const LOCKED_EVENT: &str = "vault://locked";
/// Items were added or changed from outside the UI (the browser extension).
pub const ITEMS_CHANGED_EVENT: &str = "vault://items-changed";

/// Whether this device has a server session. Independent of the lock state:
/// a locked vault is never online, and an unlocked one may be offline
/// (spec 2026-09-20 §8.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connectivity {
    Offline,
    /// Never constructed yet: nothing obtains a server session until the
    /// sync client exists (spec 2026-09-20 §13 step 3). Kept as a variant,
    /// not deleted, so `is_online`/`require_online` do not need to change
    /// shape when it starts being set.
    #[allow(dead_code)]
    Online,
}

pub struct AppState {
    /// Shared with the browser bridge, which answers extension requests.
    vault: Arc<Mutex<VaultService>>,
    bridge: Bridge,
    lock_manager: Mutex<LockManager>,
    pub clipboard: ClipboardGuard,
    /// The export file the user picked for the last import, so the UI can
    /// offer to delete it without ever supplying a path itself.
    pub last_import: Mutex<Option<PathBuf>>,
    /// This computer's ID and Secret Key (`device.json`).
    pub device: Mutex<Device>,
    /// Whether this device currently has a server session. There is no sync
    /// client yet, so this never becomes `Online`; that is the real state of
    /// a device with no server to talk to, not a placeholder.
    connectivity: Mutex<Connectivity>,
    origin: Instant,
}

/// Error returned to the renderer: a stable code and a fixed message.
#[derive(Debug, Serialize)]
pub struct CmdError {
    pub code: &'static str,
    pub message: String,
}

impl From<havenkeys_core::Error> for CmdError {
    fn from(e: havenkeys_core::Error) -> Self {
        Self {
            code: e.code(),
            message: e.to_string(),
        }
    }
}

impl CmdError {
    pub fn internal() -> Self {
        Self {
            code: "internal",
            message: "Internal error.".into(),
        }
    }

    pub fn file() -> Self {
        Self {
            code: "file",
            message: "Could not read or delete the file.".into(),
        }
    }

    pub fn clipboard() -> Self {
        Self {
            code: "clipboard",
            message: "Could not access the clipboard.".into(),
        }
    }
}

pub type CmdResult<T> = Result<T, CmdError>;

#[derive(Clone, Serialize)]
struct LockedPayload {
    reason: &'static str,
}

impl AppState {
    pub fn new(vault: Arc<Mutex<VaultService>>, bridge: Bridge, device: Device) -> Self {
        Self {
            vault,
            bridge,
            lock_manager: Mutex::new(LockManager::new(None)),
            clipboard: ClipboardGuard::default(),
            last_import: Mutex::new(None),
            device: Mutex::new(device),
            connectivity: Mutex::new(Connectivity::Offline),
            origin: Instant::now(),
        }
    }

    pub fn vault(&self) -> CmdResult<MutexGuard<'_, VaultService>> {
        self.vault.lock().map_err(|_| CmdError::internal())
    }

    /// Does this device currently have a server session?
    pub fn is_online(&self) -> bool {
        matches!(
            self.connectivity.lock().map(|c| *c),
            Ok(Connectivity::Online)
        )
    }

    /// Refuse a mutating command while offline, before it touches the vault.
    pub fn require_online(&self) -> CmdResult<()> {
        if self.is_online() {
            Ok(())
        } else {
            Err(havenkeys_core::Error::Offline.into())
        }
    }

    /// Monotonic time since start (does not advance during suspend on Linux/macOS).
    pub fn mono(&self) -> Duration {
        self.origin.elapsed()
    }

    pub fn wall() -> Duration {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
    }

    pub fn now_ms() -> i64 {
        i64::try_from(Self::wall().as_millis()).unwrap_or(i64::MAX)
    }

    pub fn unix_seconds() -> u64 {
        Self::wall().as_secs()
    }

    /// Reset the idle timer.
    pub fn touch(&self) {
        if let Ok(mut lm) = self.lock_manager.lock() {
            lm.record_activity(self.mono());
        }
    }

    /// Tell connected browser extensions the vault is open.
    pub fn notify_unlocked(&self) {
        self.bridge.notify(Event::Unlocked {});
    }

    /// Start the auto-lock clock after a successful unlock/create.
    /// Caller must already hold (or have released) the vault lock; this only
    /// takes the lock-manager lock.
    pub fn arm_auto_lock(&self, auto_lock_minutes: u32) {
        if let Ok(mut lm) = self.lock_manager.lock() {
            lm.set_timeout(LockManager::timeout_from_minutes(auto_lock_minutes));
            lm.reset(self.mono(), Self::wall());
        }
    }

    /// Lock the vault, clear our clipboard contents and notify the UI.
    pub fn lock(&self, app: &AppHandle, reason: &'static str) {
        let was_open = match self.vault.lock() {
            Ok(mut v) => v.lock(),
            // A poisoned mutex means a panic mid-operation; make sure the
            // session is still dropped.
            Err(poisoned) => poisoned.into_inner().lock(),
        };
        self.clipboard.clear_if_owned(None);
        if was_open {
            self.bridge.notify(Event::Locked {});
            let _ = app.emit(LOCKED_EVENT, LockedPayload { reason });
        }
    }

    /// Called periodically by the auto-lock thread.
    pub fn auto_lock_tick(&self, app: &AppHandle) {
        let reason = {
            // Treat a poisoned mutex like a normal one: auto-lock must keep working.
            let vault = self.vault.lock().unwrap_or_else(|p| p.into_inner());
            if !vault.is_unlocked() {
                return;
            }
            let mut lm = self.lock_manager.lock().unwrap_or_else(|p| p.into_inner());
            lm.tick(self.mono(), Self::wall())
        };
        if let Some(reason) = reason {
            self.lock(app, reason.as_str());
        }
    }
}
