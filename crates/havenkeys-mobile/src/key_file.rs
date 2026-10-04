//! The Secret Key on Android: a file in app-private storage, sealed by a
//! Keystore key the app holds (spec 2026-10-01-android-app §4.1). Kotlin only
//! encrypts and decrypts bytes; where the file lives and what it holds is
//! decided here.

use havenkeys_client::key_store::{KeyStore, StoreError};
use havenkeys_core::SecretString;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Debug, uniffi::Error)]
pub enum CipherError {
    Failed,
}

impl std::fmt::Display for CipherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("keystore failure")
    }
}

impl std::error::Error for CipherError {}

impl From<uniffi::UnexpectedUniFFICallbackError> for CipherError {
    fn from(_: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::Failed
    }
}

#[uniffi::export(with_foreign)]
pub trait KeystoreCipher: Send + Sync {
    fn seal(&self, plaintext: Vec<u8>) -> Result<Vec<u8>, CipherError>;
    fn open(&self, sealed: Vec<u8>) -> Result<Vec<u8>, CipherError>;
}

pub(crate) struct KeystoreKeyStore {
    dir: PathBuf,
    cipher: Arc<dyn KeystoreCipher>,
}

impl KeystoreKeyStore {
    pub fn new(dir: PathBuf, cipher: Arc<dyn KeystoreCipher>) -> Self {
        Self { dir, cipher }
    }

    fn path(&self, account: Uuid) -> PathBuf {
        self.dir.join(format!("secret-key-{account}.bin"))
    }
}

impl KeyStore for KeystoreKeyStore {
    fn get(&self, account: Uuid) -> Result<Option<SecretString>, StoreError> {
        let sealed = match std::fs::read(self.path(account)) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(StoreError),
        };
        let plain = Zeroizing::new(self.cipher.open(sealed).map_err(|_| StoreError)?);
        let text = std::str::from_utf8(&plain).map_err(|_| StoreError)?;
        Ok(Some(SecretString::new(text.to_owned())))
    }

    fn set(&self, account: Uuid, value: &SecretString) -> Result<(), StoreError> {
        let sealed = self
            .cipher
            .seal(value.expose().as_bytes().to_vec())
            .map_err(|_| StoreError)?;
        let tmp = self.path(account).with_extension("tmp");
        std::fs::write(&tmp, sealed).map_err(|_| StoreError)?;
        std::fs::rename(&tmp, self.path(account)).map_err(|_| StoreError)
    }

    fn delete(&self, account: Uuid) -> Result<(), StoreError> {
        match std::fs::remove_file(self.path(account)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(StoreError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::tests::XorCipher;
    use havenkeys_client::key_store::KeyStore;
    use havenkeys_core::SecretString;
    use uuid::Uuid;

    #[test]
    fn the_secret_key_round_trips_and_is_not_on_disk_in_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let store = KeystoreKeyStore::new(dir.path().to_path_buf(), Arc::new(XorCipher(true)));
        let account = Uuid::from_u128(1);
        assert!(store.get(account).unwrap().is_none());
        store
            .set(account, &SecretString::from("A3-SECRET"))
            .unwrap();
        assert_eq!(store.get(account).unwrap().unwrap().expose(), "A3-SECRET");
        let raw = std::fs::read(dir.path().join(format!("secret-key-{account}.bin"))).unwrap();
        assert!(!raw.windows(9).any(|w| w == b"A3-SECRET"));
        store.delete(account).unwrap();
        store.delete(account).unwrap();
        assert!(store.get(account).unwrap().is_none());
    }

    #[test]
    fn a_cipher_that_fails_never_writes_the_key_in_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let seen = Arc::new(crate::vault::tests::Seen::default());
        let vault = crate::vault::MobileVault::new(
            crate::vault::MobileConfig {
                data_dir: dir.path().to_string_lossy().into_owned(),
                own_package: "net.havenkeys.android".into(),
                device_name: "Pixel 8".into(),
            },
            seen,
            Arc::new(XorCipher(false)),
        )
        .unwrap();
        let key = havenkeys_core::crypto::secret_key::SecretKey::generate().unwrap();
        assert!(vault
            .client
            .device()
            .unwrap()
            .set_secret_key(Uuid::from_u128(1), &key)
            .is_err());
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let bytes = std::fs::read(entry.unwrap().path()).unwrap_or_default();
            let text = key.to_text();
            assert!(!bytes
                .windows(text.expose().len())
                .any(|w| w == text.expose().as_bytes()));
        }
    }
}
