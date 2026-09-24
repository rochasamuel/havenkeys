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
use std::sync::atomic::{AtomicBool, Ordering};
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

/// Run `f` on its own thread; `None` if it does not answer within
/// `TIMEOUT` (the thread is then abandoned) or cannot be started.
fn run_timed<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("keychain".into())
        .spawn(move || {
            let _ = tx.send(f());
        })
        .ok()?;
    rx.recv_timeout(TIMEOUT).ok()
}

/// Wraps any store so that every call is bounded by `TIMEOUT`. A store that
/// once failed to answer in time is not asked again for the life of the
/// process: a hung keychain would otherwise cost every unlock five seconds
/// and leave another thread waiting on it each time.
pub struct TimedKeyStore {
    inner: Arc<dyn KeyStore>,
    gave_up: Arc<AtomicBool>,
}

impl TimedKeyStore {
    pub fn new(inner: Box<dyn KeyStore>) -> Self {
        Self {
            inner: Arc::from(inner),
            gave_up: Arc::new(AtomicBool::new(false)),
        }
    }

    fn call<T: Send + 'static>(
        &self,
        f: impl FnOnce(&dyn KeyStore) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, StoreError> {
        if self.gave_up.load(Ordering::Relaxed) {
            return Err(StoreError);
        }
        let inner = Arc::clone(&self.inner);
        match run_timed(move || f(&*inner)) {
            Some(result) => result,
            None => {
                self.gave_up.store(true, Ordering::Relaxed);
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
    available: bool,
}

impl OsKeyStore {
    /// Install the platform store as keyring-core's default. Called once at
    /// start; a failure (or no answer within the timeout) leaves every call
    /// below failing, i.e. the file fallback.
    pub fn install() -> Self {
        let installed = run_timed(|| platform_store().map(keyring_core::set_default_store));
        Self {
            available: matches!(installed, Some(Ok(()))),
        }
    }

    fn entry(&self, account: Uuid) -> Result<keyring_core::Entry, StoreError> {
        if !self.available {
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
    fn a_store_that_hangs_times_out_and_is_not_asked_again() {
        let timed = TimedKeyStore::new(Box::new(SlowKeyStore::new(Duration::from_secs(30))));
        let started = Instant::now();
        assert!(timed.get(ACCOUNT).is_err());
        let first = started.elapsed();
        assert!(first >= TIMEOUT && first < Duration::from_secs(10));
        let again = Instant::now();
        assert!(timed.set(ACCOUNT, &SecretString::from("v")).is_err());
        assert!(again.elapsed() < Duration::from_secs(1));
    }
}
