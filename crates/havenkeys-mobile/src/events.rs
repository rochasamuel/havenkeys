use havenkeys_client::ClientEvents;
use havenkeys_core::lock::{LockManager, LockReason};
use havenkeys_core::sync::SyncReport;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[uniffi::export(with_foreign)]
pub trait VaultEvents: Send + Sync {
    fn locked(&self, reason: String);
    fn unlocked(&self);
    fn connectivity(&self, online: bool);
    fn signed_out(&self);
    fn items_changed(&self);
    fn removed(&self);
    /// The account was deleted (here or on another device) and this phone's
    /// copy erased. The app deletes its Keystore keys, as for `removed`.
    fn account_deleted(&self);
}

enum Event {
    Locked(&'static str),
    Unlocked,
    Connectivity(bool),
    SignedOut,
    ItemsChanged,
    Removed,
    AccountDeleted,
}

/// Hands the client's events to the app from a thread of their own:
/// `ClientEvents::unlocked` runs under the vault guard, and an app listener
/// that called back into the vault there would deadlock.
pub(crate) struct EventPump(Mutex<Sender<Event>>);

impl EventPump {
    fn start(app: Arc<dyn VaultEvents>) -> Self {
        let (tx, rx) = channel::<Event>();
        let _ = std::thread::Builder::new()
            .name("vault-events".into())
            .spawn(move || {
                for event in rx {
                    match event {
                        Event::Locked(r) => app.locked(r.to_owned()),
                        Event::Unlocked => app.unlocked(),
                        Event::Connectivity(o) => app.connectivity(o),
                        Event::SignedOut => app.signed_out(),
                        Event::ItemsChanged => app.items_changed(),
                        Event::Removed => app.removed(),
                        Event::AccountDeleted => app.account_deleted(),
                    }
                }
            });
        Self(Mutex::new(tx))
    }

    fn send(&self, e: Event) {
        if let Ok(tx) = self.0.lock() {
            let _ = tx.send(e);
        }
    }
}

/// The auto-lock clock: the core's `LockManager` on a monotonic origin.
pub(crate) struct LockClock {
    manager: Mutex<LockManager>,
    origin: Instant,
}

impl LockClock {
    pub fn new() -> Self {
        Self {
            manager: Mutex::new(LockManager::new(None)),
            origin: Instant::now(),
        }
    }

    fn wall() -> Duration {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
    }

    pub fn arm(&self, minutes: u32) {
        if let Ok(mut m) = self.manager.lock() {
            m.set_timeout(LockManager::timeout_from_minutes(minutes));
            m.reset(self.origin.elapsed(), Self::wall());
        }
    }

    /// Records activity unless the vault is already overdue: a late touch
    /// must not rescue it. Returns the reason to lock now, if any.
    pub fn touch(&self) -> Option<LockReason> {
        let mut m = self.manager.lock().unwrap_or_else(|p| p.into_inner());
        let now = self.origin.elapsed();
        let due = m.tick(now, Self::wall());
        if due.is_none() {
            m.record_activity(now);
        }
        due
    }

    pub fn due(&self) -> Option<LockReason> {
        let mut m = self.manager.lock().unwrap_or_else(|p| p.into_inner());
        m.tick(self.origin.elapsed(), Self::wall())
    }

    /// Tests only: a timeout shorter than the one-minute settings allow.
    #[cfg(test)]
    pub fn arm_for(&self, timeout: Duration) {
        let mut m = self.manager.lock().unwrap_or_else(|p| p.into_inner());
        m.set_timeout(Some(timeout));
        m.reset(self.origin.elapsed(), Self::wall());
    }
}

pub(crate) struct MobileEvents {
    pump: EventPump,
    clock: Arc<LockClock>,
}

impl MobileEvents {
    pub fn new(app: Arc<dyn VaultEvents>, clock: Arc<LockClock>) -> Self {
        Self {
            pump: EventPump::start(app),
            clock,
        }
    }
}

impl ClientEvents for MobileEvents {
    fn unlocked(&self, auto_lock_minutes: u32) {
        self.clock.arm(auto_lock_minutes);
        self.pump.send(Event::Unlocked);
    }

    fn locked(&self, reason: &'static str, was_open: bool) {
        if was_open {
            self.pump.send(Event::Locked(reason));
        }
    }

    fn connectivity(&self, online: bool) {
        self.pump.send(Event::Connectivity(online));
    }

    fn signed_out(&self) {
        self.pump.send(Event::SignedOut);
    }

    fn synced(&self, _report: SyncReport) {
        self.pump.send(Event::ItemsChanged);
    }

    fn items_changed(&self) {
        self.pump.send(Event::ItemsChanged);
    }

    fn removed(&self, _keychain_cleared: bool) {
        self.pump.send(Event::Removed);
    }

    fn account_deleted(&self, _keychain_cleared: bool) {
        self.pump.send(Event::AccountDeleted);
    }
}
