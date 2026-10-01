use havenkeys_core::sync::SyncReport;

/// What the shell is told. Implementations must return quickly.
///
/// `unlocked` is called while the vault guard is held, so that a concurrent
/// lock can never be overtaken by an unlock announcement: it must not block
/// and must not touch the vault. `locked` is called just after the guard is
/// released, so an implementation may not assume the vault is still locked.
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
