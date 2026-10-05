//! Deleting the account (spec 2026-10-05-account-deletion §5): the server
//! erases everything first; only then does this device erase its copy.
//!
//! The local wipe is guarded by a marker file written before it starts, so
//! a wipe cut short (crash, power loss, a locked file) is finished at the
//! next start instead of leaving an orphan replica of a deleted account.

use crate::client::HavenClient;
use crate::error::{ClientError, ClientResult};
use crate::removal::confirms;
use havenkeys_core::store::Store;
use havenkeys_core::vault;
use havenkeys_core::SecretString;
use havenkeys_sync_client::SyncError;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::task::spawn_blocking;
use uuid::Uuid;

fn marker_path(vault_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.pending-deletion", vault_path.display()))
}

impl HavenClient {
    /// Delete the account on the server, then this device's copy.
    ///
    /// Unlocked and online, checked here: the shell is not trusted to gate
    /// this. The master password is checked locally first, so a typo is
    /// "wrong password" and never reaches the server's rate limit or looks
    /// like a dead session.
    pub async fn delete_account(
        self: &Arc<Self>,
        confirmation: String,
        master_password: SecretString,
    ) -> ClientResult<()> {
        self.require_unlocked()?;
        self.require_online()?;
        let record = self
            .vault()?
            .account()?
            .ok_or(havenkeys_core::Error::NoVault)?;
        if !confirms(&confirmation, &record.email) {
            return Err(havenkeys_core::Error::InvalidInput(
                "type this account's email to confirm",
            )
            .into());
        }
        self.verify_master_password(master_password.clone()).await?;

        let account = self.vault_account()?;
        let secret_key = self
            .device()?
            .secret_key(account.id)
            .ok_or(havenkeys_core::Error::SecretKeyRequired)?;
        let kdf = self.vault()?.kdf()?.ok_or(havenkeys_core::Error::NoVault)?;
        let email = account.email.as_str().to_owned();
        let auth_key = spawn_blocking(move || {
            vault::derive_auth_key(&master_password, &secret_key, &kdf, &account)
        })
        .await
        .map_err(|_| ClientError::internal())??;

        let (session, server) = (self.session()?, self.server()?);
        let sent = server.delete_account(&session, &auth_key, &email).await;
        drop(auth_key);
        match sent {
            // Already gone (deleted from another device meanwhile): same end.
            Ok(()) | Err(SyncError::AccountDeleted) => {}
            Err(e) => return Err(self.failed(e)),
        }

        let client = Arc::clone(self);
        let account_id = record.account_id;
        spawn_blocking(move || client.wipe_local(account_id))
            .await
            .map_err(|_| ClientError::internal())?;
        Ok(())
    }

    /// Finish a wipe a previous run started. Call once at startup, right
    /// after construction. Returns whether one ran.
    pub fn finish_pending_deletion(&self) -> bool {
        let marker = marker_path(&self.config.vault_path);
        let Ok(text) = std::fs::read_to_string(&marker) else {
            return false;
        };
        // Only the account the marker names, or a vault already emptied: a
        // stale marker must never erase an account signed in since.
        let current = self
            .vault()
            .ok()
            .and_then(|v| v.account().ok().flatten())
            .map(|a| a.account_id);
        match text.trim().parse::<Uuid>() {
            Ok(account_id) if current.is_none_or(|c| c == account_id) => {
                self.wipe_local(account_id);
                true
            }
            _ => {
                let _ = std::fs::remove_file(&marker);
                false
            }
        }
    }

    /// Erase this device's copy of a deleted account: the vault file (and
    /// SQLite's sidecars), the Secret Key, and the device id. Files set
    /// aside by an earlier "remove this device" are not touched: they may
    /// belong to another account.
    ///
    /// Synchronous and bounded (the key store has its own timeout), so it
    /// can run from `failed()`. Every step runs even if an earlier one
    /// failed; the marker is removed only once the file is gone.
    pub(crate) fn wipe_local(&self, account_id: Uuid) {
        let path = self.config.vault_path.clone();
        let marker = marker_path(&path);
        let _ = std::fs::write(&marker, account_id.to_string());

        self.lock("account_deleted");
        self.forget_server();

        let mut file_gone = false;
        if let Ok(mut vault) = self.vault() {
            if let Ok(empty) = Store::open_in_memory() {
                drop(vault.replace_store(empty)); // closes the file
            }
            for suffix in ["-wal", "-shm", "-journal"] {
                let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
            }
            file_gone = match std::fs::remove_file(&path) {
                Ok(()) => true,
                Err(e) => e.kind() == std::io::ErrorKind::NotFound,
            };
            if let Ok(store) = Store::open(&path) {
                let _ = vault.replace_store(store);
            }
        }

        let keychain_cleared = match self.device() {
            Ok(mut device) => device.forget(account_id).keychain_cleared,
            Err(_) => false,
        };
        if file_gone {
            let _ = std::fs::remove_file(&marker);
        }
        self.events.account_deleted(keychain_cleared);
    }
}

#[cfg(test)]
mod tests {
    use crate::client::tests::{client_in, RecordingEvents};
    use crate::client::HavenClient;
    use havenkeys_core::account::{AccountRef, NormalizedEmail};
    use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
    use havenkeys_core::store::{AccountRecord, Store};
    use havenkeys_core::vault::prepare_new_account_vault;
    use havenkeys_core::SecretString;
    use std::path::Path;
    use std::sync::Arc;

    const PASSWORD: &str = "correct horse battery staple";

    /// An unlocked account vault on disk at `dir/vault.sqlite3`, offline.
    fn account_vault(dir: &Path) -> (Arc<HavenClient>, Arc<RecordingEvents>, uuid::Uuid) {
        let path = dir.join("vault.sqlite3");
        let (client, events) = client_in(dir);
        client
            .vault()
            .unwrap()
            .replace_store(Store::open(&path).unwrap());
        let account = AccountRef::new(
            uuid::Uuid::from_u128(3),
            NormalizedEmail::parse("user@example.com").unwrap(),
        );
        let made = prepare_new_account_vault(
            &SecretString::from(PASSWORD),
            &account,
            KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap(),
            1_700_000_000_000,
        )
        .unwrap();
        client
            .vault()
            .unwrap()
            .create_account_vault(
                made.prepared,
                &AccountRecord {
                    account_id: account.id,
                    email: "user@example.com".into(),
                    server_url: "http://127.0.0.1:9".into(),
                    server_cursor: 0,
                    max_header_rev: 0,
                    last_synced_at: None,
                },
            )
            .unwrap();
        client
            .device()
            .unwrap()
            .set_secret_key(account.id, &made.secret_key)
            .unwrap();
        (client, events, account.id)
    }

    fn marker(dir: &Path) -> std::path::PathBuf {
        dir.join("vault.sqlite3.pending-deletion")
    }

    #[tokio::test]
    async fn a_locked_vault_cannot_be_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _) = client_in(dir.path());
        let err = client
            .delete_account("user@example.com".into(), SecretString::from(PASSWORD))
            .await
            .unwrap_err();
        assert_eq!(err.code, "locked");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn offline_deletes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _, id) = account_vault(dir.path());
        let err = client
            .delete_account("user@example.com".into(), SecretString::from(PASSWORD))
            .await
            .unwrap_err();
        assert_eq!(err.code, "offline");
        assert!(client.vault().unwrap().account().unwrap().is_some());
        assert!(client.device().unwrap().secret_key(id).is_some());
    }

    #[test]
    fn deletion_wipes_file_key_and_announces() {
        let dir = tempfile::tempdir().unwrap();
        let (client, events, id) = account_vault(dir.path());
        let old_device = client.device_id().unwrap();
        client.wipe_local(id);
        // A fresh, empty store is open at the path: no account in it.
        assert!(client.vault().unwrap().account().unwrap().is_none());
        assert!(client.device().unwrap().secret_key(id).is_none());
        assert_ne!(client.device_id().unwrap(), old_device);
        assert!(!marker(dir.path()).exists());
        assert!(events.seen().contains(&"account_deleted:true".to_string()));
        assert!(!events.seen().iter().any(|e| e.starts_with("removed")));
    }

    #[test]
    fn a_pending_marker_is_finished_at_start() {
        let dir = tempfile::tempdir().unwrap();
        let (client, events, id) = account_vault(dir.path());
        std::fs::write(marker(dir.path()), id.to_string()).unwrap();
        assert!(client.finish_pending_deletion());
        assert!(client.device().unwrap().secret_key(id).is_none());
        assert!(client.vault().unwrap().account().unwrap().is_none());
        assert!(events.seen().contains(&"account_deleted:true".to_string()));
        assert!(!client.finish_pending_deletion(), "nothing left to finish");
    }

    #[test]
    fn a_garbled_marker_is_removed_and_nothing_is_wiped() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _, id) = account_vault(dir.path());
        std::fs::write(marker(dir.path()), "not a uuid").unwrap();
        assert!(!client.finish_pending_deletion());
        assert!(client.device().unwrap().secret_key(id).is_some());
        assert!(!marker(dir.path()).exists());
    }

    #[test]
    fn a_stale_marker_never_erases_another_account() {
        let dir = tempfile::tempdir().unwrap();
        let (client, events, id) = account_vault(dir.path());
        let other = uuid::Uuid::from_u128(99);
        std::fs::write(marker(dir.path()), other.to_string()).unwrap();
        assert!(!client.finish_pending_deletion());
        assert!(client.vault().unwrap().account().unwrap().is_some());
        assert!(client.device().unwrap().secret_key(id).is_some());
        assert!(!marker(dir.path()).exists());
        assert!(!events
            .seen()
            .iter()
            .any(|e| e.starts_with("account_deleted")));
    }

    #[test]
    fn set_aside_files_survive_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let older = dir.path().join("vault.sqlite3.removed-20260101T000000Z");
        std::fs::write(&older, b"someone's last copy").unwrap();
        let (client, _, id) = account_vault(dir.path());
        client.wipe_local(id);
        assert!(older.exists());
    }

    #[test]
    fn a_410_wipes_the_device() {
        let dir = tempfile::tempdir().unwrap();
        let (client, events, id) = account_vault(dir.path());
        let err = client.failed(havenkeys_sync_client::SyncError::AccountDeleted);
        assert_eq!(err.code, "account_deleted");
        assert!(client.device().unwrap().secret_key(id).is_none());
        assert!(events.seen().contains(&"account_deleted:true".to_string()));
    }
}
