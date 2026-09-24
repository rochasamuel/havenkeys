//! What this computer knows about itself, kept outside the vault database:
//! a random device ID and the Secret Key.
//!
//! The Secret Key lives in the OS keychain (`secret_store.rs`), under the
//! account's ID. `device.json` lives next to `vault.sqlite3` in the app's
//! data folder (mode 0600 on Unix) and holds the device ID; it holds the
//! Secret Key too, in plain text, only when no keychain answered — which
//! Settings shows as a warning. Either way, keeping the Secret Key out of the
//! vault database means a copied or backed-up vault file is useless without
//! the Emergency Kit. See docs/crypto.md, "Secret Key".
//!
//! Only one Secret Key is kept in `device.json`, without an account: a
//! device holds one vault, and the key there is the one for that vault's
//! account.

use crate::secret_store::{KeyStore, Storage, TimedKeyStore};
use havenkeys_core::crypto::secret_key::SecretKey;
use havenkeys_core::SecretString;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use zeroize::Zeroizing;

const FILE: &str = "device.json";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OnDisk {
    device_id: Uuid,
    #[serde(default)]
    secret_key: Option<String>,
}

/// What was last found for one account, so status calls do not ask the
/// keychain every time. Replaced by every write.
struct Cached {
    account: Uuid,
    text: Option<SecretString>,
    storage: Storage,
}

pub struct Device {
    path: PathBuf,
    pub id: Uuid,
    /// The key kept in `device.json`, when no keychain took it.
    file_key: Option<SecretString>,
    cached: Option<Cached>,
    /// Always timed: no call here can hang the caller.
    store: TimedKeyStore,
}

fn parses(s: &SecretString) -> bool {
    SecretKey::parse(s.expose()).is_ok()
}

impl Device {
    /// Load `device.json` from `dir`, creating it with a new device ID if it
    /// is missing. An unreadable file is replaced: losing it only means the
    /// Secret Key must be entered again from the Emergency Kit, if it was
    /// not in the keychain. Reads nothing from the keychain yet.
    pub fn load(dir: &Path, store: Box<dyn KeyStore>) -> Self {
        let path = dir.join(FILE);
        let parsed = std::fs::read(&path)
            .ok()
            .map(Zeroizing::new)
            .and_then(|b| serde_json::from_slice::<OnDisk>(&b).ok());
        let (id, file_key) = match parsed {
            // A value that does not parse is treated as absent.
            Some(d) => (
                d.device_id,
                d.secret_key.map(SecretString::new).filter(parses),
            ),
            None => (Uuid::new_v4(), None),
        };
        let mut device = Device {
            path,
            id,
            file_key,
            cached: None,
            store: TimedKeyStore::new(store),
        };
        let _ = device.save();
        device
    }

    /// Find the key for `account`: the keychain first, then the file. A
    /// keychain that failed is not cached, so the next call asks again
    /// (unless the timed store has given up on it).
    fn resolve(&mut self, account: Uuid) -> (Option<SecretString>, Storage) {
        if let Some(c) = self.cached.as_ref().filter(|c| c.account == account) {
            return (c.text.clone(), c.storage);
        }
        let (text, storage, definite) = match self.store.get(account) {
            Ok(Some(v)) if parses(&v) => (Some(v), Storage::Keychain, true),
            from_keychain => match &self.file_key {
                Some(k) => (Some(k.clone()), Storage::File, true),
                None => (None, Storage::None, from_keychain.is_ok()),
            },
        };
        self.cached = definite.then(|| Cached {
            account,
            text: text.clone(),
            storage,
        });
        (text, storage)
    }

    pub fn secret_key(&mut self, account: Uuid) -> Option<SecretKey> {
        self.secret_key_text(account)
            .and_then(|s| SecretKey::parse(s.expose()).ok())
    }

    pub fn secret_key_text(&mut self, account: Uuid) -> Option<SecretString> {
        self.resolve(account).0
    }

    /// Where the key for `account` is kept.
    pub fn storage(&mut self, account: Uuid) -> Storage {
        self.resolve(account).1
    }

    /// Keep `key` for `account`: in the keychain if it takes the key and
    /// gives the same value back, otherwise in `device.json`. Returns where
    /// it went.
    pub fn set_secret_key(&mut self, account: Uuid, key: &SecretKey) -> std::io::Result<Storage> {
        let text = key.to_text();
        let in_keychain = self.store.set(account, &text).is_ok()
            && matches!(self.store.get(account), Ok(Some(v)) if v.expose() == text.expose());
        let (file_key, storage) = if in_keychain {
            (None, Storage::Keychain)
        } else {
            (Some(text.clone()), Storage::File)
        };
        let previous = std::mem::replace(&mut self.file_key, file_key);
        if let Err(e) = self.save() {
            // The file still holds what it held; so does memory.
            self.file_key = previous;
            self.cached = None;
            return Err(e);
        }
        self.cached = Some(Cached {
            account,
            text: Some(text),
            storage,
        });
        Ok(storage)
    }

    /// Move a key left in device.json into the keychain, if one is available.
    pub fn migrate(&mut self, account: Uuid) {
        let Some(key) = self
            .file_key
            .as_ref()
            .and_then(|s| SecretKey::parse(s.expose()).ok())
        else {
            return;
        };
        // On failure the key stays in the file, where it already was.
        let _ = self.set_secret_key(account, &key);
    }

    /// Remove the key everywhere and start over with a new device id.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "called by \"remove this device\" (next task)")
    )]
    pub fn forget(&mut self, account: Uuid) -> std::io::Result<()> {
        let _ = self.store.delete(account);
        self.file_key = None;
        self.cached = None;
        self.id = Uuid::new_v4();
        self.save()
    }

    /// Write atomically with owner-only permissions.
    fn save(&mut self) -> std::io::Result<()> {
        let data = OnDisk {
            device_id: self.id,
            secret_key: self.file_key.as_ref().map(|s| s.expose().to_owned()),
        };
        let bytes =
            Zeroizing::new(serde_json::to_vec_pretty(&data).map_err(std::io::Error::other)?);
        drop(data);
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut f = private_create(&tmp)?;
            f.write_all(&bytes)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &self.path)
    }
}

#[cfg(unix)]
fn private_create(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn private_create(path: &Path) -> std::io::Result<std::fs::File> {
    // The app data folder is per-user on Windows.
    std::fs::File::create(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret_store::{FailingKeyStore, KeyStore, MemoryKeyStore, SlowKeyStore, Storage};

    const ACCOUNT: Uuid = Uuid::from_u128(7);

    fn key() -> SecretKey {
        SecretKey::generate().unwrap()
    }

    fn file_text(dir: &Path) -> String {
        std::fs::read_to_string(dir.join("device.json")).unwrap()
    }

    #[test]
    fn a_key_goes_to_the_keychain_and_not_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Device::load(dir.path(), Box::new(MemoryKeyStore::default()));
        let k = key();
        assert_eq!(d.set_secret_key(ACCOUNT, &k).unwrap(), Storage::Keychain);
        assert!(!file_text(dir.path()).contains(k.to_text().expose()));
        assert!(!file_text(dir.path()).contains("secretKey\": \"H1"));
        assert_eq!(
            d.secret_key_text(ACCOUNT).unwrap().expose(),
            k.to_text().expose()
        );
    }

    #[test]
    fn without_a_keychain_the_key_falls_back_to_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Device::load(dir.path(), Box::new(FailingKeyStore));
        let k = key();
        assert_eq!(d.set_secret_key(ACCOUNT, &k).unwrap(), Storage::File);
        assert!(file_text(dir.path()).contains(k.to_text().expose()));
        assert_eq!(d.storage(ACCOUNT), Storage::File);
    }

    #[test]
    fn a_keychain_that_hangs_falls_back_within_the_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Device::load(
            dir.path(),
            Box::new(SlowKeyStore::new(std::time::Duration::from_secs(30))),
        );
        let started = std::time::Instant::now();
        assert_eq!(d.set_secret_key(ACCOUNT, &key()).unwrap(), Storage::File);
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
    }

    #[test]
    fn a_key_in_the_file_is_moved_to_the_keychain() {
        let dir = tempfile::tempdir().unwrap();
        let k = key();
        {
            let mut d = Device::load(dir.path(), Box::new(FailingKeyStore));
            d.set_secret_key(ACCOUNT, &k).unwrap();
        }
        let store = MemoryKeyStore::default();
        let mut d = Device::load(dir.path(), Box::new(store.clone()));
        d.migrate(ACCOUNT);
        assert!(!file_text(dir.path()).contains(k.to_text().expose()));
        assert_eq!(
            store.get(ACCOUNT).unwrap().unwrap().expose(),
            k.to_text().expose()
        );
        assert_eq!(d.storage(ACCOUNT), Storage::Keychain);
    }

    #[test]
    fn forget_removes_the_key_everywhere_and_renews_the_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryKeyStore::default();
        let mut d = Device::load(dir.path(), Box::new(store.clone()));
        let old_id = d.id;
        d.set_secret_key(ACCOUNT, &key()).unwrap();
        d.forget(ACCOUNT).unwrap();
        assert!(store.get(ACCOUNT).unwrap().is_none());
        assert!(d.secret_key(ACCOUNT).is_none());
        assert_ne!(d.id, old_id);
        assert_eq!(d.storage(ACCOUNT), Storage::None);
    }
}
