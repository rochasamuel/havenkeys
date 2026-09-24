//! Shared application state.
//!
//! Lock ordering (to avoid deadlocks): `vault` before `lock_manager`, and
//! `vault` before the bridge's connection list (`notify` may run under the
//! vault lock; the bridge never takes the vault while holding its own locks).

use crate::clipboard::ClipboardGuard;
use crate::device::Device;
use crate::sync::Client;
use havenkeys_bridge::Bridge;
use havenkeys_core::lock::LockManager;
use havenkeys_core::vault::VaultService;
use havenkeys_protocol::Event;
use havenkeys_sync_client::Session;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

pub const LOCKED_EVENT: &str = "vault://locked";
/// Items were added or changed from outside the UI (the browser extension).
pub const ITEMS_CHANGED_EVENT: &str = "vault://items-changed";

/// Whether this device has a server session. Independent of the lock state:
/// a locked vault is never online, and an unlocked one may be offline
/// (spec 2026-09-20 §8.6).
pub enum Connectivity {
    Offline,
    /// The token lives here and nowhere else — never on disk — and is
    /// dropped (and zeroized) when the vault locks or the device goes
    /// offline.
    Online(Session),
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
    /// Whether this device currently has a server session.
    connectivity: Mutex<Connectivity>,
    /// The HTTP client for this vault's server, kept because building one
    /// sets up a TLS stack. Keyed by URL so a re-pointed vault cannot keep
    /// talking to the old server.
    sync_client: Mutex<Option<(String, Client)>>,
    /// When a pull was last attempted, so the periodic one keeps its spacing
    /// whether or not the attempt worked. Real sync times live in the
    /// account record.
    last_sync_attempt: Mutex<Option<Duration>>,
    /// Set when the vault file on this computer could not be opened. The app
    /// still starts — it has to, or there is nowhere to show the reason —
    /// and every command that touches the vault refuses with this.
    storage_error: Option<CmdError>,
    origin: Instant,
    /// The app's data folder: where `vault.sqlite3` and `device.json` live.
    data_dir: PathBuf,
}

/// Error returned to the renderer: a stable code and a fixed message.
#[derive(Clone, Debug, Serialize)]
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

    /// One message for every way signing in can fail. The device never says
    /// whether it was the address, the password or the Secret Key.
    pub fn sign_in_failed() -> Self {
        Self {
            code: "sign_in_failed",
            message: "Email, master password or Secret Key is incorrect.".into(),
        }
    }

    /// The vault file exists but this build cannot open it. Carries the
    /// folder so the message can tell the user where their file is; a path
    /// is not a secret, and without it the advice is unfollowable.
    pub fn vault_unreadable(dir: &std::path::Path) -> Self {
        Self {
            code: "vault_unreadable",
            message: format!(
                "This vault was created by an older version of HavenKeys and cannot be \
                 opened by this one. Your file is in {}. Move vault.sqlite3 and \
                 device.json somewhere safe — do not delete them — and start HavenKeys \
                 again to set this computer up with an invite.",
                dir.display()
            ),
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
    /// `storage_error` is set when the vault file could not be opened: the
    /// app then runs on an empty in-memory store so the window can open and
    /// say so, and every command refuses before touching it.
    pub fn new(
        vault: Arc<Mutex<VaultService>>,
        bridge: Bridge,
        device: Device,
        storage_error: Option<CmdError>,
        data_dir: PathBuf,
    ) -> Self {
        Self {
            vault,
            bridge,
            lock_manager: Mutex::new(LockManager::new(None)),
            clipboard: ClipboardGuard::default(),
            last_import: Mutex::new(None),
            device: Mutex::new(device),
            connectivity: Mutex::new(Connectivity::Offline),
            sync_client: Mutex::new(None),
            last_sync_attempt: Mutex::new(None),
            storage_error,
            origin: Instant::now(),
            data_dir,
        }
    }

    /// The app's data folder: where `vault.sqlite3` and `device.json` live.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Drop the cached HTTP client, so a stale one for an old server is
    /// never reused after this device is removed from its account.
    pub fn forget_sync_client(&self) {
        if let Ok(mut cached) = self.sync_client.lock() {
            *cached = None;
        }
    }

    pub fn vault(&self) -> CmdResult<MutexGuard<'_, VaultService>> {
        // Checked here rather than in each command: this is the one door
        // every one of them goes through.
        if let Some(err) = &self.storage_error {
            return Err(err.clone());
        }
        self.vault.lock().map_err(|_| CmdError::internal())
    }

    /// Does this device currently have a server session?
    pub fn is_online(&self) -> bool {
        matches!(
            self.connectivity.lock().as_deref(),
            Ok(Connectivity::Online(_))
        )
    }

    /// The session for an authenticated request, or `Offline`.
    pub fn session(&self) -> CmdResult<Session> {
        match self.connectivity.lock().as_deref() {
            Ok(Connectivity::Online(session)) => Ok(session.clone()),
            _ => Err(havenkeys_core::Error::Offline.into()),
        }
    }

    pub fn set_online(&self, session: Session) {
        if let Ok(mut c) = self.connectivity.lock() {
            *c = Connectivity::Online(session);
        }
    }

    /// Drop the session. Returns whether this changed anything, so the caller
    /// only tells the UI when it did.
    pub fn go_offline(&self) -> bool {
        match self.connectivity.lock() {
            Ok(mut c) => {
                let was_online = matches!(*c, Connectivity::Online(_));
                *c = Connectivity::Offline;
                was_online
            }
            Err(_) => false,
        }
    }

    pub fn device_id(&self) -> CmdResult<uuid::Uuid> {
        Ok(self.device.lock().map_err(|_| CmdError::internal())?.id)
    }

    /// The cached HTTP client for `url`, building one on first use.
    pub fn sync_client(
        &self,
        url: &str,
        build: impl FnOnce() -> CmdResult<Client>,
    ) -> CmdResult<Client> {
        let mut cached = self.sync_client.lock().map_err(|_| CmdError::internal())?;
        if let Some((cached_url, client)) = cached.as_ref() {
            if cached_url == url {
                return Ok(client.clone());
            }
        }
        let client = build()?;
        *cached = Some((url.to_string(), client.clone()));
        Ok(client)
    }

    pub fn mark_sync_attempt(&self) {
        if let Ok(mut last) = self.last_sync_attempt.lock() {
            *last = Some(self.mono());
        }
    }

    /// Is a periodic pull due? Also true when none has been attempted yet.
    pub fn sync_due(&self, interval: Duration) -> bool {
        match self.last_sync_attempt.lock() {
            Ok(last) => last.is_none_or(|at| self.mono().saturating_sub(at) >= interval),
            Err(_) => false,
        }
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
        // Locked means no session: the token is dropped (and zeroized) here,
        // so a locked vault cannot reach the server at all (design §6).
        self.go_offline();
        if let Ok(mut last) = self.last_sync_attempt.lock() {
            *last = None;
        }
        // The path of the last imported `.1pux` is only there so the UI can
        // offer to delete it after an import. A locked vault has no import in
        // progress, so the offer — and the ability to act on it — goes away.
        if let Ok(mut last) = self.last_import.lock() {
            *last = None;
        }
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
