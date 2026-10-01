//! The server session, and every operation that needs one.
//!
//! The vault is a replica: reads come from SQLite and work offline, writes go
//! to the server first and are recorded locally only once it has accepted
//! them (spec 2026-09-20 §8.4).
//!
//! Nothing here holds the vault lock across an `await`: the guard is taken,
//! used and dropped before every network call, so a lock is never delayed by
//! a slow server.

use crate::client::HavenClient;
use crate::error::{ClientError, ClientResult};
use crate::now_ms;
use havenkeys_core::crypto::keys::AuthKey;
use havenkeys_core::model::ItemOverview;
use havenkeys_core::sync::SyncReport;
use havenkeys_core::vault::StagedWrite;
use havenkeys_sync_client::SyncError;
use std::time::Duration;
use uuid::Uuid;

/// How often an unlocked, online device pulls.
pub const PULL_INTERVAL: Duration = Duration::from_secs(60);

/// The most changes the server takes in one request (spec 2026-09-20 §7.3).
pub const MAX_BATCH: usize = 500;

impl HavenClient {
    /// Map a failure, and drop the session when the server says it is gone,
    /// so the device falls back to read-only instead of retrying with a dead
    /// token. A refused session is announced as signed out, not just offline.
    pub(crate) fn failed(&self, err: SyncError) -> ClientError {
        if matches!(err, SyncError::Unauthorized | SyncError::Unavailable) {
            let dropped = self.go_offline();
            if dropped && err == SyncError::Unauthorized {
                self.events.signed_out();
            }
        }
        err.into()
    }

    /// Sign in to the account with a freshly derived auth key, then catch up.
    ///
    /// A failure here is not an unlock failure: the vault stays open and
    /// readable, offline.
    pub async fn connect(&self, auth_key: AuthKey) -> ClientResult<()> {
        let (email, account_id, device_id, server) = {
            let vault = self.vault()?;
            if !vault.is_unlocked() {
                return Err(havenkeys_core::Error::Locked.into());
            }
            let account = vault.account()?.ok_or(havenkeys_core::Error::NoVault)?;
            drop(vault);
            let server = self.server_for(&account.server_url)?;
            (account.email, account.account_id, self.device_id()?, server)
        };

        let session = server
            .login(
                &email,
                &auth_key,
                account_id,
                device_id,
                self.config.device_name,
            )
            .await
            .map_err(|e| {
                // No session is held here as a rule, so `failed` would not
                // announce this one.
                if e == SyncError::Unauthorized {
                    self.events.signed_out();
                }
                self.failed(e)
            })?;
        drop(auth_key);

        // A lock that landed while signing in wins: the session is dropped
        // unused rather than left on a locked vault.
        {
            let vault = self.vault()?;
            if !vault.is_unlocked() {
                return Err(havenkeys_core::Error::Locked.into());
            }
            self.set_online(session);
        }
        self.events.connectivity(true);
        self.sync_now().await?;
        self.ensure_identity().await;
        Ok(())
    }

    /// Create the account's Identity if the replica does not have it (spec
    /// 2026-09-29-identity-item §5.2). Runs after the pull, so "missing" means
    /// missing on the server too, unless another device is creating it right
    /// now: then the server refuses this write and a pull brings theirs. Never
    /// fails the caller; the next connect tries again.
    async fn ensure_identity(&self) {
        let staged = {
            let Ok(vault) = self.vault() else { return };
            let Ok(Some(account)) = vault.account() else {
                return;
            };
            match vault.stage_identity_if_missing(&account.email, now_ms()) {
                Ok(Some(staged)) => staged,
                _ => return,
            }
        };
        match self.push(staged).await {
            Ok(_) => self.events.items_changed(),
            Err(e) if e.code == "item_changed_elsewhere" => {
                if self.sync_now().await.is_ok() {
                    self.events.items_changed();
                }
            }
            Err(_) => {}
        }
    }

    /// Reconcile the header, then pull until the replica has caught up.
    pub async fn sync_now(&self) -> ClientResult<SyncReport> {
        // Recorded up front, so a failing server is retried on the same
        // spacing rather than on every tick.
        self.mark_sync_attempt();
        let (session, server) = (self.session()?, self.server()?);
        let mut report = SyncReport::default();

        // The header first: a master password changed on another device must
        // be adopted before anything else. Nothing is ever published from
        // here. The core checks the attestation and refuses a revision that
        // goes backwards.
        let remote = server.header(&session).await.map_err(|e| self.failed(e))?;
        {
            let revision = self.vault()?.header_revision()?.unwrap_or(0);
            if remote.revision as u64 > revision {
                report.header_adopted = self.vault()?.adopt_account_header(&remote.bytes)?;
            } else if revision > remote.revision as u64 {
                // Ahead of the server means something is wrong locally; send
                // nothing.
                return Err(ClientError::internal());
            }
        }

        let mut cursor = self
            .vault()?
            .account()?
            .map(|a| a.server_cursor)
            .unwrap_or(0);
        loop {
            let pulled = server
                .pull(&session, cursor)
                .await
                .map_err(|e| self.failed(e))?;
            let has_more = pulled.has_more;
            let next = pulled.cursor;
            let page = self
                .vault()?
                .apply_remote_changes(next, pulled.changes, now_ms())?;
            report.added += page.added;
            report.updated += page.updated;
            report.deleted += page.deleted;
            report.skipped_items += page.skipped_items;

            // A server claiming there is more while handing out the same
            // cursor would spin this loop forever.
            if !has_more || next <= cursor {
                break;
            }
            cursor = next;
        }

        // Items that did not open earlier are asked for again, by id, until
        // they open, are deleted, or a Re-download clears them. Best effort:
        // a failure stops the retry for this sync and nothing more. Only a
        // refused session is acted on.
        let pending = self.vault()?.unreadable_item_ids()?;
        for chunk in pending.chunks(MAX_BATCH) {
            let changes = match server.fetch_items(&session, chunk).await {
                Ok(changes) => changes,
                Err(SyncError::Unauthorized) => {
                    let _ = self.failed(SyncError::Unauthorized);
                    break;
                }
                Err(_) => break,
            };
            let applied = self
                .vault()
                .and_then(|mut v| Ok(v.apply_refetched(chunk, changes, now_ms())?));
            let Ok(page) = applied else { break };
            report.added += page.added;
            report.updated += page.updated;
            report.deleted += page.deleted;
        }

        self.mark_sync_attempt();
        self.events.synced(report);
        Ok(report)
    }

    /// Send one staged write, then record what the server accepted. Nothing
    /// is written locally until the server has assigned the revision, so the
    /// replica can never be ahead of the authority.
    pub async fn push(&self, staged: StagedWrite) -> ClientResult<Option<ItemOverview>> {
        let (session, server) = (self.session()?, self.server()?);
        let item_id = staged.item_id;
        let ack = server
            .write(&session, std::slice::from_ref(&staged))
            .await
            .map_err(|e| self.failed(e))?;
        let revision = revision_for(&ack.applied, item_id)?;
        Ok(self.vault()?.commit_write(staged, revision)?)
    }

    /// Send many staged writes, in batches the server accepts, committing
    /// each batch before the next is sent. Returns how many were recorded: a
    /// refused batch stops the run, and earlier batches are already on the
    /// server.
    pub async fn push_batches(&self, staged: Vec<StagedWrite>) -> ClientResult<usize> {
        let (session, server) = (self.session()?, self.server()?);
        let mut committed = 0usize;
        let mut queue = staged;
        while !queue.is_empty() {
            let rest = queue.split_off(queue.len().min(MAX_BATCH));
            let batch = std::mem::replace(&mut queue, rest);
            let ack = server
                .write(&session, &batch)
                .await
                .map_err(|e| self.failed(e))?;
            for write in batch {
                let revision = revision_for(&ack.applied, write.item_id)?;
                self.vault()?.commit_write(write, revision)?;
                committed += 1;
            }
        }
        Ok(committed)
    }
}

fn revision_for(applied: &[(Uuid, i64)], item_id: Uuid) -> ClientResult<i64> {
    applied
        .iter()
        .find(|(id, _)| *id == item_id)
        .map(|(_, revision)| *revision)
        .ok_or_else(|| ClientError {
            code: "sync_failed",
            message: "The server did not acknowledge that item.".into(),
        })
}

#[cfg(test)]
mod tests {
    use crate::client::tests::client_in;
    use crate::client::HavenClient;
    use crate::stub_server::{StubAccount, StubServer};
    use havenkeys_core::account::{AccountRef, NormalizedEmail};
    use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
    use havenkeys_core::crypto::keys::AuthKey;
    use havenkeys_core::store::AccountRecord;
    use havenkeys_core::vault::prepare_new_account_vault;
    use havenkeys_core::SecretString;
    use uuid::Uuid;

    /// Give `client` an unlocked account vault pointing at `server_url`, and
    /// return the auth key `connect` signs in with.
    fn open_account_vault(client: &HavenClient, server_url: &str) -> AuthKey {
        let account = AccountRef::new(
            Uuid::from_u128(1),
            NormalizedEmail::parse("user@example.com").unwrap(),
        );
        let kdf = KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap();
        let made = prepare_new_account_vault(
            &SecretString::from("correct horse battery staple"),
            &account,
            kdf,
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
                    server_url: server_url.into(),
                    server_cursor: 0,
                    max_header_rev: 0,
                    last_synced_at: None,
                },
            )
            .unwrap();
        made.auth_key
    }

    #[tokio::test]
    async fn connect_on_a_locked_vault_stays_offline() {
        let dir = tempfile::tempdir().unwrap();
        let (client, _) = client_in(dir.path());
        // Nothing listens here: a request would fail as offline, not as
        // locked.
        let auth_key = open_account_vault(&client, "http://127.0.0.1:9");
        client.lock("user");

        let err = client.connect(auth_key).await.unwrap_err();
        assert_eq!(err.code, "locked");
        assert!(!client.is_online());
    }

    #[tokio::test]
    async fn a_lock_during_login_drops_the_new_session() {
        let server = StubServer::start(StubAccount {
            account_id: Uuid::from_u128(1),
            vault_id: Uuid::from_u128(2),
            kdf: KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap(),
            header: Vec::new(),
            header_revision: 0,
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let (client, events) = client_in(dir.path());
        let auth_key = open_account_vault(&client, &server.url);

        let connecting = tokio::spawn({
            let client = client.clone();
            async move { client.connect(auth_key).await }
        });
        // The vault was unlocked when `connect` started, so only the check
        // after the login can catch this lock.
        server.login_entered.notified().await;
        client.lock("auto");
        server.release_login.notify_one();

        let err = connecting.await.unwrap().unwrap_err();
        assert_eq!(err.code, "locked");
        assert!(!client.is_online());
        assert!(!events.seen().iter().any(|e| e == "online:true"));
    }
}
