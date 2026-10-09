//! Shared application state.
//!
//! Lock ordering (to avoid deadlocks): `vault` before `lock_manager`, and
//! `vault` before the bridge's connection list (`notify` may run under the
//! vault lock; the bridge never takes the vault while holding its own locks).

use crate::clipboard::ClipboardGuard;
use crate::item_input::ItemInputWire;
use crate::qr_scan::ScannedCode;
use crate::scan_slot::{ScanSlot, ScannedTotp};
use havenkeys_bridge::Bridge;
use havenkeys_client::HavenClient;
pub use havenkeys_client::{ClientError as CmdError, ClientResult as CmdResult};
use havenkeys_core::lock::LockManager;
use havenkeys_core::model::ItemInput;
use havenkeys_core::vault::VaultService;
use havenkeys_protocol::Event;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const LOCKED_EVENT: &str = "vault://locked";
/// Items were added or changed from outside the UI (the browser extension).
pub const ITEMS_CHANGED_EVENT: &str = "vault://items-changed";
/// The browser extension asked to edit this item (payload: its UUID).
pub const OPEN_ITEM_EVENT: &str = "vault://open-item";

pub struct AppState {
    client: Arc<HavenClient>,
    /// The same vault the client holds, for the auto-lock tick, which must
    /// keep working on a poisoned mutex.
    vault: Arc<Mutex<VaultService>>,
    bridge: Bridge,
    lock_manager: Mutex<LockManager>,
    pub clipboard: ClipboardGuard,
    /// The export file the user picked for the last import, so the UI can
    /// offer to delete it without ever supplying a path itself.
    pub last_import: Mutex<Option<PathBuf>>,
    /// The last QR scan's codes, waiting to be saved (scan_slot.rs).
    totp_scan: Mutex<ScanSlot>,
    origin: Instant,
}

impl AppState {
    pub fn new(client: Arc<HavenClient>, vault: Arc<Mutex<VaultService>>, bridge: Bridge) -> Self {
        Self {
            client,
            vault,
            bridge,
            lock_manager: Mutex::new(LockManager::new(None)),
            clipboard: ClipboardGuard::default(),
            last_import: Mutex::new(None),
            totp_scan: Mutex::new(ScanSlot::default()),
            origin: Instant::now(),
        }
    }

    /// Hold a scan's codes. The vault guard is held throughout so this is
    /// ordered against `lock()`, which clears the slot after it has locked
    /// the vault and released that guard: a scan that finishes as the vault
    /// locks leaves nothing behind.
    pub fn store_totp_scan(&self, codes: Vec<ScannedCode>) -> CmdResult<Vec<ScannedTotp>> {
        let vault = self.vault()?;
        if !vault.is_unlocked() {
            return Err(havenkeys_core::Error::Locked.into());
        }
        let mut slot = self.totp_scan.lock().map_err(|_| CmdError::internal())?;
        Ok(slot.add(codes, Instant::now())?)
    }

    /// The core's `ItemInput` for a renderer request, with a scanned token
    /// replaced by its URI.
    pub fn resolve_item_input(&self, input: ItemInputWire) -> CmdResult<ItemInput> {
        let slot = self.totp_scan.lock().map_err(|_| CmdError::internal())?;
        input.resolve(&slot, Instant::now())
    }

    pub fn clear_totp_scan(&self) {
        let mut slot = self.totp_scan.lock().unwrap_or_else(|p| p.into_inner());
        slot.clear();
    }

    pub fn client(&self) -> &Arc<HavenClient> {
        &self.client
    }

    pub fn vault(&self) -> CmdResult<MutexGuard<'_, VaultService>> {
        self.client.vault()
    }

    pub fn require_unlocked(&self) -> CmdResult<()> {
        self.client.require_unlocked()
    }

    pub fn require_online(&self) -> CmdResult<()> {
        self.client.require_online()
    }

    /// Refuses writes while the account is frozen (trial ended or payment
    /// lapsed). Reads, export and sign-in are not gated.
    pub fn require_full(&self) -> CmdResult<()> {
        self.client.require_full()
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
        havenkeys_client::now_ms()
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

    pub fn notify_locked(&self) {
        self.bridge.notify(Event::Locked {});
    }

    /// What locking clears on the desktop side. Called by `DesktopEvents`
    /// on every lock, whether or not the vault was open.
    pub fn clear_after_lock(&self) {
        self.clipboard.clear_if_owned(None);
        // The path of the last imported `.1pux` is only there so the UI can
        // offer to delete it after an import. A locked vault has no import in
        // progress, so the offer — and the ability to act on it — goes away.
        if let Ok(mut last) = self.last_import.lock() {
            *last = None;
        }
        // Scanned TOTP codes waiting to be saved are vault secrets too.
        self.clear_totp_scan();
    }

    pub fn lock(&self, reason: &'static str) {
        self.client.lock(reason);
    }

    /// Called periodically by the auto-lock thread.
    pub fn auto_lock_tick(&self) {
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
            self.lock(reason.as_str());
        }
    }
}
