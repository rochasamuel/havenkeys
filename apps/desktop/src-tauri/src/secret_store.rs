//! Where the Secret Key lives on this computer: the OS keychain (Windows
//! Credential Manager, macOS Keychain, the Secret Service on Linux), or,
//! when none is available, `device.json` (see `device.rs`).
//!
//! Nothing here returns or logs the value in an error: `StoreError` has no
//! payload. Every keychain call runs on its own thread with a timeout
//! (`TimedKeyStore`), because on a Linux desktop with no Secret Service a
//! D-Bus call can wait a long time, and the file fallback is better than a
//! frozen unlock screen. `Device` always wraps its store in `TimedKeyStore`,
//! so the timeout applies to every store, the test fakes included.

use havenkeys_core::SecretString;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// The keychain entry's service name; the account ID is the user name.
const SERVICE: &str = "app.havenkeys";
/// How long one keychain call may take before the file fallback is used.
const TIMEOUT: Duration = Duration::from_secs(5);

/// Where this computer keeps the Secret Key for an account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Storage {
    Keychain,
    File,
    None,
}

/// A keychain failure. Deliberately carries nothing: never the value, and
/// not the platform's message either, which may quote attributes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct StoreError;

impl std::fmt::Debug for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StoreError")
    }
}

/// A place to keep one secret per account.
pub trait KeyStore: Send + Sync {
    /// `Ok(None)` when the store works but has no entry for `account`.
    fn get(&self, account: Uuid) -> Result<Option<SecretString>, StoreError>;
    fn set(&self, account: Uuid, value: &SecretString) -> Result<(), StoreError>;
    /// Deleting an entry that does not exist is not an error.
    fn delete(&self, account: Uuid) -> Result<(), StoreError>;
}

/// Why `run_timed` has no result.
enum NoAnswer {
    /// The thread could not be started; `f` never ran.
    NotStarted,
    /// `f` is still running on its (now abandoned) thread.
    TimedOut,
}

/// Run `f` on its own thread and wait at most `timeout` for it.
fn run_timed<T: Send + 'static>(
    timeout: Duration,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, NoAnswer> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("keychain".into())
        .spawn(move || {
            let _ = tx.send(f());
        })
        .map_err(|_| NoAnswer::NotStarted)?;
    rx.recv_timeout(timeout).map_err(|_| NoAnswer::TimedOut)
}

/// Wraps any store so that every call is bounded by `TIMEOUT`.
///
/// A call that did not answer in time keeps running on its thread (it may be
/// waiting on a keyring unlock prompt the user has not answered yet). While
/// it runs the store is *busy*: further calls fail at once instead of
/// waiting five seconds each and stacking up threads. Once the abandoned
/// call returns, the store is asked again as normal, so a slow prompt never
/// downgrades a working keychain for the rest of the session.
pub struct TimedKeyStore {
    inner: Arc<dyn KeyStore>,
    busy: Arc<AtomicBool>,
    timeout: Duration,
}

impl TimedKeyStore {
    pub fn new(inner: Box<dyn KeyStore>) -> Self {
        Self::with_timeout(inner, TIMEOUT)
    }

    fn with_timeout(inner: Box<dyn KeyStore>, timeout: Duration) -> Self {
        Self {
            inner: Arc::from(inner),
            busy: Arc::new(AtomicBool::new(false)),
            timeout,
        }
    }

    fn call<T: Send + 'static>(
        &self,
        f: impl FnOnce(&dyn KeyStore) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, StoreError> {
        // One call at a time; a pending (possibly abandoned) one fails the
        // rest fast.
        if self.busy.swap(true, Ordering::AcqRel) {
            return Err(StoreError);
        }
        let inner = Arc::clone(&self.inner);
        let busy = Arc::clone(&self.busy);
        let result = run_timed(self.timeout, move || {
            let r = f(&*inner);
            // Cleared before the result is sent, so a caller that got it can
            // make its next call straight away.
            busy.store(false, Ordering::Release);
            r
        });
        match result {
            Ok(r) => r,
            // Still running: its thread clears the flag when it returns.
            Err(NoAnswer::TimedOut) => Err(StoreError),
            Err(NoAnswer::NotStarted) => {
                self.busy.store(false, Ordering::Release);
                Err(StoreError)
            }
        }
    }
}

impl KeyStore for TimedKeyStore {
    fn get(&self, account: Uuid) -> Result<Option<SecretString>, StoreError> {
        self.call(move |s| s.get(account))
    }

    fn set(&self, account: Uuid, value: &SecretString) -> Result<(), StoreError> {
        let value = value.clone();
        self.call(move |s| s.set(account, &value))
    }

    fn delete(&self, account: Uuid) -> Result<(), StoreError> {
        self.call(move |s| s.delete(account))
    }
}

/// The platform keychain through keyring-core. Calls here block without a
/// limit; `Device` only ever uses it through `TimedKeyStore`.
pub struct OsKeyStore {
    /// One of the `PLATFORM_*` values. Shared with the install thread, which
    /// may finish after `install` stopped waiting for it.
    platform: Arc<AtomicU8>,
}

/// This build has no keychain for this OS: a definite "nothing stored".
const PLATFORM_NONE: u8 = 0;
/// Installing the platform store has not finished (it timed out and is
/// still running). The keychain may well hold the key.
const PLATFORM_PENDING: u8 = 1;
/// The platform store is installed as keyring-core's default.
const PLATFORM_READY: u8 = 2;
/// Installing the platform store failed (for example the Secret Service did
/// not answer on D-Bus). Not a definite answer either: it may be transient.
const PLATFORM_FAILED: u8 = 3;

impl OsKeyStore {
    /// Install the platform store as keyring-core's default. Called once at
    /// start, and waits at most `TIMEOUT`. An install that is still running
    /// then finishes in the background and the store becomes usable when it
    /// does; until then, and after a failure, every call fails (the caller
    /// treats that as "the keychain did not answer", never as "no key").
    pub fn install() -> Self {
        if !cfg!(any(target_os = "linux", windows, target_os = "macos")) {
            return Self::with_platform(PLATFORM_NONE);
        }
        let platform = Arc::new(AtomicU8::new(PLATFORM_PENDING));
        let done = Arc::clone(&platform);
        // Whether it answered in time or not, the thread records the result.
        let installed = run_timed(TIMEOUT, move || {
            let state = match platform_store().map(keyring_core::set_default_store) {
                Ok(()) => PLATFORM_READY,
                Err(_) => PLATFORM_FAILED,
            };
            done.store(state, Ordering::Release);
        });
        if let Err(NoAnswer::NotStarted) = installed {
            platform.store(PLATFORM_FAILED, Ordering::Release);
        }
        Self { platform }
    }

    fn with_platform(state: u8) -> Self {
        Self {
            platform: Arc::new(AtomicU8::new(state)),
        }
    }

    fn platform(&self) -> u8 {
        self.platform.load(Ordering::Acquire)
    }

    fn entry(&self, account: Uuid) -> Result<keyring_core::Entry, StoreError> {
        if self.platform() != PLATFORM_READY {
            return Err(StoreError);
        }
        keyring_core::Entry::new(SERVICE, &account.to_string()).map_err(|_| StoreError)
    }
}

#[cfg(target_os = "linux")]
fn platform_store() -> Result<Arc<keyring_core::CredentialStore>, StoreError> {
    let store = zbus_secret_service_keyring_store::Store::new().map_err(|_| StoreError)?;
    Ok(store)
}

#[cfg(windows)]
fn platform_store() -> Result<Arc<keyring_core::CredentialStore>, StoreError> {
    let store = windows_native_keyring_store::Store::new().map_err(|_| StoreError)?;
    Ok(store)
}

#[cfg(target_os = "macos")]
fn platform_store() -> Result<Arc<keyring_core::CredentialStore>, StoreError> {
    let store = apple_native_keyring_store::keychain::Store::new().map_err(|_| StoreError)?;
    Ok(store)
}

#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
fn platform_store() -> Result<Arc<keyring_core::CredentialStore>, StoreError> {
    Err(StoreError)
}

impl KeyStore for OsKeyStore {
    fn get(&self, account: Uuid) -> Result<Option<SecretString>, StoreError> {
        // No keychain for this OS holds nothing: a definite answer, so a
        // device without one is asked for its Secret Key as usual. A store
        // that failed to install, or is still installing, is not: it may
        // hold the key, so that is an error, never `Ok(None)`.
        if self.platform() == PLATFORM_NONE {
            return Ok(None);
        }
        match self.entry(account)?.get_password() {
            Ok(v) => Ok(Some(SecretString::new(v))),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(_) => Err(StoreError),
        }
    }

    fn set(&self, account: Uuid, value: &SecretString) -> Result<(), StoreError> {
        self.entry(account)?
            .set_password(value.expose())
            .map_err(|_| StoreError)
    }

    fn delete(&self, account: Uuid) -> Result<(), StoreError> {
        // Nothing can be stored without a keychain; with one that did not
        // install, a deletion cannot be confirmed.
        if self.platform() == PLATFORM_NONE {
            return Ok(());
        }
        match self.entry(account)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(_) => Err(StoreError),
        }
    }
}

// ------------------------------------------------------------------ test fakes

/// An in-memory keychain. Clones share their entries.
#[cfg(test)]
#[derive(Clone, Default)]
pub struct MemoryKeyStore(Arc<std::sync::Mutex<std::collections::HashMap<Uuid, String>>>);

#[cfg(test)]
impl KeyStore for MemoryKeyStore {
    fn get(&self, account: Uuid) -> Result<Option<SecretString>, StoreError> {
        let map = self.0.lock().map_err(|_| StoreError)?;
        Ok(map.get(&account).cloned().map(SecretString::new))
    }

    fn set(&self, account: Uuid, value: &SecretString) -> Result<(), StoreError> {
        let mut map = self.0.lock().map_err(|_| StoreError)?;
        map.insert(account, value.expose().to_owned());
        Ok(())
    }

    fn delete(&self, account: Uuid) -> Result<(), StoreError> {
        let mut map = self.0.lock().map_err(|_| StoreError)?;
        map.remove(&account);
        Ok(())
    }
}

/// No keychain at all: every call fails.
#[cfg(test)]
pub struct FailingKeyStore;

#[cfg(test)]
impl KeyStore for FailingKeyStore {
    fn get(&self, _: Uuid) -> Result<Option<SecretString>, StoreError> {
        Err(StoreError)
    }

    fn set(&self, _: Uuid, _: &SecretString) -> Result<(), StoreError> {
        Err(StoreError)
    }

    fn delete(&self, _: Uuid) -> Result<(), StoreError> {
        Err(StoreError)
    }
}

/// A working keychain whose first call hangs for `delay` (a prompt the user
/// is slow to answer); every call after that answers at once.
#[cfg(test)]
#[derive(Clone)]
pub struct SlowOnceKeyStore {
    delay: Duration,
    calls: Arc<std::sync::atomic::AtomicUsize>,
    finished: Arc<AtomicBool>,
    inner: MemoryKeyStore,
}

#[cfg(test)]
impl SlowOnceKeyStore {
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
            calls: Default::default(),
            finished: Default::default(),
            inner: MemoryKeyStore::default(),
        }
    }

    /// How many calls reached this store.
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// Whether the slow first call has returned.
    pub fn slow_call_finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }

    fn enter(&self) {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            std::thread::sleep(self.delay);
            self.finished.store(true, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
impl KeyStore for SlowOnceKeyStore {
    fn get(&self, account: Uuid) -> Result<Option<SecretString>, StoreError> {
        self.enter();
        self.inner.get(account)
    }

    fn set(&self, account: Uuid, value: &SecretString) -> Result<(), StoreError> {
        self.enter();
        self.inner.set(account, value)
    }

    fn delete(&self, account: Uuid) -> Result<(), StoreError> {
        self.enter();
        self.inner.delete(account)
    }
}

/// A keychain that hangs for `delay`, then succeeds (storing nothing).
#[cfg(test)]
pub struct SlowKeyStore(Duration);

#[cfg(test)]
impl SlowKeyStore {
    pub fn new(delay: Duration) -> Self {
        Self(delay)
    }
}

#[cfg(test)]
impl KeyStore for SlowKeyStore {
    fn get(&self, _: Uuid) -> Result<Option<SecretString>, StoreError> {
        std::thread::sleep(self.0);
        Ok(None)
    }

    fn set(&self, _: Uuid, _: &SecretString) -> Result<(), StoreError> {
        std::thread::sleep(self.0);
        Ok(())
    }

    fn delete(&self, _: Uuid) -> Result<(), StoreError> {
        std::thread::sleep(self.0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    const ACCOUNT: Uuid = Uuid::from_u128(7);

    #[test]
    fn a_store_error_prints_nothing_but_its_name() {
        assert_eq!(format!("{:?}", StoreError), "StoreError");
    }

    #[test]
    fn storage_serializes_in_lowercase() {
        assert_eq!(
            serde_json::to_string(&[Storage::Keychain, Storage::File, Storage::None]).unwrap(),
            r#"["keychain","file","none"]"#
        );
    }

    #[test]
    fn the_timed_store_passes_calls_through() {
        let inner = MemoryKeyStore::default();
        let timed = TimedKeyStore::new(Box::new(inner.clone()));
        timed.set(ACCOUNT, &SecretString::from("v")).unwrap();
        assert_eq!(inner.get(ACCOUNT).unwrap().unwrap().expose(), "v");
        assert_eq!(timed.get(ACCOUNT).unwrap().unwrap().expose(), "v");
        timed.delete(ACCOUNT).unwrap();
        assert!(timed.get(ACCOUNT).unwrap().is_none());
    }

    #[test]
    fn a_store_that_hangs_times_out_at_the_real_timeout() {
        let timed = TimedKeyStore::new(Box::new(SlowKeyStore::new(Duration::from_secs(30))));
        let started = Instant::now();
        assert!(timed.get(ACCOUNT).is_err());
        let first = started.elapsed();
        assert!(first >= TIMEOUT && first < Duration::from_secs(10));
    }

    fn wait_until(what: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !what() {
            assert!(Instant::now() < deadline, "timed out waiting");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn while_a_timed_out_call_is_pending_calls_fail_fast_without_reaching_the_store() {
        let slow = SlowOnceKeyStore::new(Duration::from_millis(1500));
        let timed = TimedKeyStore::with_timeout(Box::new(slow.clone()), Duration::from_millis(100));
        assert!(timed.get(ACCOUNT).is_err());
        assert!(!slow.slow_call_finished());
        let again = Instant::now();
        assert!(timed.set(ACCOUNT, &SecretString::from("v")).is_err());
        assert!(timed.get(ACCOUNT).is_err());
        assert!(again.elapsed() < Duration::from_millis(100));
        // Neither reached the store: no thread was started for them.
        assert_eq!(slow.calls(), 1);
    }

    #[test]
    fn once_the_timed_out_call_returns_the_store_is_asked_again() {
        let slow = SlowOnceKeyStore::new(Duration::from_millis(300));
        let timed = TimedKeyStore::with_timeout(Box::new(slow.clone()), Duration::from_millis(50));
        assert!(timed.get(ACCOUNT).is_err());
        wait_until(|| slow.slow_call_finished());
        // The worker clears the busy flag just after the fake returns.
        wait_until(|| timed.set(ACCOUNT, &SecretString::from("v")).is_ok());
        assert_eq!(timed.get(ACCOUNT).unwrap().unwrap().expose(), "v");
    }

    #[test]
    fn no_platform_keychain_is_a_definite_answer() {
        let os = OsKeyStore::with_platform(PLATFORM_NONE);
        assert!(os.get(ACCOUNT).unwrap().is_none());
        assert!(os.delete(ACCOUNT).is_ok());
        assert!(os.set(ACCOUNT, &SecretString::from("v")).is_err());
    }

    #[test]
    fn a_keychain_that_failed_or_is_still_installing_is_not_a_definite_answer() {
        for state in [PLATFORM_PENDING, PLATFORM_FAILED] {
            let os = OsKeyStore::with_platform(state);
            assert!(os.get(ACCOUNT).is_err(), "get must not say \"no key\"");
            assert!(os.delete(ACCOUNT).is_err(), "delete must not claim success");
            assert!(os.set(ACCOUNT, &SecretString::from("v")).is_err());
        }
    }

    #[test]
    fn back_to_back_calls_do_not_trip_the_busy_flag() {
        let timed = TimedKeyStore::new(Box::new(MemoryKeyStore::default()));
        for _ in 0..200 {
            timed.set(ACCOUNT, &SecretString::from("v")).unwrap();
            assert!(timed.get(ACCOUNT).unwrap().is_some());
        }
    }
}
