//! Where the Secret Key lives on this computer: the OS keychain (Windows
//! Credential Manager, macOS Keychain, the Secret Service on Linux), or,
//! when none is available, `device.json` (see `havenkeys_client::device`).
//! The `KeyStore` trait and the timeout wrapper live in `havenkeys-client`.

use havenkeys_client::key_store::{run_timed, KeyStore, NoAnswer, StoreError};
use havenkeys_core::SecretString;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// The keychain entry's service name; the account ID is the user name.
const SERVICE: &str = "app.havenkeys";
/// How long the platform store may take to install before the file fallback is used.
const TIMEOUT: Duration = Duration::from_secs(5);

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

#[cfg(test)]
mod tests {
    use super::*;

    const ACCOUNT: Uuid = Uuid::from_u128(7);

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
}
