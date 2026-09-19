//! What this computer knows about itself, kept outside the vault database:
//! a random device ID, the Secret Key, and the sync folder.
//!
//! `device.json` lives next to `vault.sqlite3` in the app's data folder (mode
//! 0600 on Unix). The Secret Key is stored here in plain text, as 1Password
//! does on its devices: someone who can read this computer's files already
//! has the vault file, and then only the master password protects it. What
//! the Secret Key protects is every copy that is *not* on a trusted device:
//! the sync folder, and backups of `vault.sqlite3`. Keeping it out of the
//! vault database means a copied or backed-up vault file is useless without
//! the Emergency Kit. See docs/crypto.md, "Secret Key".

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
    #[serde(default)]
    sync_folder: Option<PathBuf>,
}

pub struct Device {
    path: PathBuf,
    pub id: Uuid,
    secret_key: Option<SecretString>,
    pub sync_folder: Option<PathBuf>,
}

impl Device {
    /// Load `device.json` from `dir`, creating it with a new device ID if it
    /// is missing. An unreadable file is replaced: losing it only means the
    /// Secret Key must be entered again from the Emergency Kit.
    pub fn load(dir: &Path) -> Self {
        let path = dir.join(FILE);
        let parsed = std::fs::read(&path)
            .ok()
            .map(Zeroizing::new)
            .and_then(|b| serde_json::from_slice::<OnDisk>(&b).ok());
        let mut device = match parsed {
            Some(d) => Device {
                path,
                id: d.device_id,
                // A value that does not parse is treated as absent.
                secret_key: d
                    .secret_key
                    .map(SecretString::new)
                    .filter(|s| SecretKey::parse(s.expose()).is_ok()),
                sync_folder: d.sync_folder,
            },
            None => Device {
                path,
                id: Uuid::new_v4(),
                secret_key: None,
                sync_folder: None,
            },
        };
        let _ = device.save();
        device
    }

    pub fn secret_key(&self) -> Option<SecretKey> {
        self.secret_key
            .as_ref()
            .and_then(|s| SecretKey::parse(s.expose()).ok())
    }

    pub fn secret_key_text(&self) -> Option<&SecretString> {
        self.secret_key.as_ref()
    }

    pub fn set_secret_key(&mut self, key: &SecretKey) -> std::io::Result<()> {
        self.secret_key = Some(key.to_text());
        self.save()
    }

    pub fn set_sync_folder(&mut self, folder: Option<PathBuf>) -> std::io::Result<()> {
        self.sync_folder = folder;
        self.save()
    }

    /// Write atomically with owner-only permissions.
    pub fn save(&mut self) -> std::io::Result<()> {
        let data = OnDisk {
            device_id: self.id,
            secret_key: self.secret_key.as_ref().map(|s| s.expose().to_owned()),
            sync_folder: self.sync_folder.clone(),
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
