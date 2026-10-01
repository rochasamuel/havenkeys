use havenkeys_core::sync::SyncReport;

/// What the shell is told. Implementations must return quickly and must not
/// take the vault: `unlocked` and `locked` run while it is held, so that a
/// concurrent lock can never be overtaken by an unlock announcement.
pub trait ClientEvents: Send + Sync {
    /// The vault opened; start the auto-lock clock.
    fn unlocked(&self, auto_lock_minutes: u32);
    /// The vault locked (or a lock was requested on a locked vault:
    /// `was_open` is false). Clear anything that held secrets.
    fn locked(&self, reason: &'static str, was_open: bool);
    fn connectivity(&self, online: bool);
    /// The server refused this device's session.
    fn signed_out(&self);
    fn synced(&self, report: SyncReport);
    fn items_changed(&self);
    /// This device left its account; `keychain_cleared` is false when the
    /// Secret Key may still be in the platform store.
    fn removed(&self, keychain_cleared: bool);
}
