//! Restoring an encrypted backup through the server (spec 2026-10-05-export
//! §6).
//!
//! The server keeps a tombstone `(revision, deleted)` for every item ever
//! deleted, and the replica drops the row outright. A backup item deleted
//! after the backup was made is therefore "new" to the replica but not to the
//! server, which refuses a create (`baseRevision = null`) for an ID it has
//! seen, and refuses the whole batch with it. Each conflict is resolved per
//! item here:
//!
//! * a **tombstone** (the server reports the row deleted at revision `r`) is
//!   revived by resending that item with `baseRevision = r`, which the
//!   server accepts only while the row is still that tombstone;
//! * a **live** item (another device wrote that ID after this one pulled) is
//!   left alone and counted as `skipped_existing` — restore never overwrites.
//!
//! The batch is then resent without the live ones, so one conflicting item
//! never sinks the rest.

use crate::client::HavenClient;
use crate::error::ClientResult;
use crate::now_ms;
use crate::sync::{revision_for, MAX_BATCH};
use havenkeys_core::export::backup::OpenedBackup;
use havenkeys_core::import::ImportReport;
use havenkeys_core::vault::{StagedImport, StagedWrite};
use havenkeys_sync_client::SyncError;
use std::collections::HashSet;
use uuid::Uuid;

impl HavenClient {
    /// Pull, stage every backup item not already here, and send them. The
    /// report counts what the server accepted.
    pub async fn restore_backup(&self, backup: OpenedBackup) -> ClientResult<ImportReport> {
        // Fresh first: "already here" is decided against the replica.
        self.sync_now().await?;
        let staged = self.vault()?.stage_restore(backup, now_ms())?;
        self.push_restore(staged).await
    }

    /// Send staged restore writes in batches, resolving conflicts per item
    /// (see the module docs). A failure other than a conflict stops the run;
    /// earlier batches are already on the server and in the replica.
    pub async fn push_restore(&self, staged: StagedImport) -> ClientResult<ImportReport> {
        let StagedImport { writes, mut report } = staged;
        if writes.is_empty() {
            report.imported = 0;
            return Ok(report);
        }
        let (session, server) = (self.session()?, self.server()?);
        let mut committed = 0usize;
        let mut queue = writes;
        while !queue.is_empty() {
            let rest = queue.split_off(queue.len().min(MAX_BATCH));
            let mut batch = std::mem::replace(&mut queue, rest);
            // An item is revived at most once: a second conflict means the
            // tombstone moved on (someone else wrote it), so it is left alone.
            // Every round removes or revives at least one item, so this ends.
            let mut revived: HashSet<Uuid> = HashSet::new();
            while !batch.is_empty() {
                let conflicts = match server.write(&session, &batch).await {
                    Ok(ack) => {
                        for write in batch.drain(..) {
                            let revision = revision_for(&ack.applied, write.item_id)?;
                            self.vault()?.commit_write(write, revision)?;
                            committed += 1;
                        }
                        break;
                    }
                    Err(SyncError::Conflict(conflicts)) => conflicts,
                    Err(e) => return Err(self.failed(e)),
                };
                let named: Vec<Uuid> = conflicts.iter().map(|c| c.item_id).collect();
                if named
                    .iter()
                    .any(|id| !batch.iter().any(|w| w.item_id == *id))
                {
                    return Err(self.failed(SyncError::Protocol(
                        "a conflict naming an item that was not sent",
                    )));
                }
                // What the server holds for each first-time conflict: only a
                // row it reports deleted is a tombstone to revive.
                let ask: Vec<Uuid> = named
                    .iter()
                    .copied()
                    .filter(|id| !revived.contains(id))
                    .collect();
                let tombstones: Vec<(Uuid, i64)> = if ask.is_empty() {
                    Vec::new()
                } else {
                    server
                        .fetch_items(&session, &ask)
                        .await
                        .map_err(|e| self.failed(e))?
                        .changes
                        .into_iter()
                        .filter(|c| c.deleted)
                        .map(|c| (c.item_id, c.revision))
                        .collect()
                };
                for id in named {
                    let tombstone = tombstones
                        .iter()
                        .find(|(t, _)| *t == id && !revived.contains(t))
                        .map(|(_, revision)| *revision);
                    match tombstone {
                        Some(revision) => {
                            if let Some(write) = batch.iter_mut().find(|w| w.item_id == id) {
                                write.base_revision = Some(revision);
                            }
                            revived.insert(id);
                        }
                        None => {
                            let skipped = take(&mut batch, id);
                            report.restore_found_existing(skipped.and_then(|w| w.item_type()));
                        }
                    }
                }
            }
        }
        report.imported = committed;
        Ok(report)
    }
}

fn take(batch: &mut Vec<StagedWrite>, id: Uuid) -> Option<StagedWrite> {
    let at = batch.iter().position(|w| w.item_id == id)?;
    Some(batch.remove(at))
}

#[cfg(test)]
mod tests {
    use crate::client::tests::client_in;
    use crate::client::HavenClient;
    use crate::stub_server::{StubAccount, StubRow, StubServer};
    use havenkeys_core::account::{AccountRef, NormalizedEmail};
    use havenkeys_core::crypto::kdf::{KdfParams, MIN_ITERATIONS, MIN_MEMORY_KIB};
    use havenkeys_core::export::backup::{open_backup, seal_backup, OpenedBackup};
    use havenkeys_core::export::{self, ExportFormat};
    use havenkeys_core::model::{ItemInput, ItemType, SecretField, SecretUpdate};
    use havenkeys_core::store::AccountRecord;
    use havenkeys_core::sync::encode_header_for;
    use havenkeys_core::vault::prepare_new_account_vault;
    use havenkeys_core::SecretString;
    use std::sync::Arc;
    use uuid::Uuid;

    const BACKUP_PW: &str = "a separate backup passphrase";

    fn fast_kdf() -> KdfParams {
        KdfParams::with_cost(MIN_MEMORY_KIB, MIN_ITERATIONS, 1).unwrap()
    }

    /// A signed-in client against a stub server that keeps items with the
    /// real server's tombstone rules.
    async fn online() -> (tempfile::TempDir, Arc<HavenClient>, StubServer) {
        let account = AccountRef::new(
            Uuid::from_u128(1),
            NormalizedEmail::parse("user@example.com").unwrap(),
        );
        let made = prepare_new_account_vault(
            &SecretString::from("correct horse battery staple"),
            &account,
            fast_kdf(),
            1_700_000_000_000,
        )
        .unwrap();
        let server = StubServer::start(StubAccount {
            account_id: account.id,
            vault_id: made.prepared.vault_id(),
            kdf: made.prepared.kdf().clone(),
            header: encode_header_for(&made.prepared).unwrap(),
            header_revision: made.prepared.header_revision() as i64,
        })
        .await;
        server.release_login.notify_one();
        let dir = tempfile::tempdir().unwrap();
        let (client, _) = client_in(dir.path());
        client
            .vault()
            .unwrap()
            .create_account_vault(
                made.prepared,
                &AccountRecord {
                    account_id: account.id,
                    email: "user@example.com".into(),
                    server_url: server.url.clone(),
                    server_cursor: 0,
                    max_header_rev: 0,
                    last_synced_at: None,
                },
            )
            .unwrap();
        client.connect(made.auth_key).await.unwrap();
        (dir, client, server)
    }

    fn login(title: &str, password: &str) -> ItemInput {
        ItemInput {
            password: SecretUpdate::Set(SecretString::from(password)),
            ..ItemInput::blank_for_tests(ItemType::Login, title)
        }
    }

    async fn create(client: &HavenClient, input: ItemInput) -> Uuid {
        let w = client.vault().unwrap().stage_create(input, 1).unwrap();
        client.push(w).await.unwrap().unwrap().id
    }

    async fn delete(client: &HavenClient, id: Uuid) {
        let w = client.vault().unwrap().stage_delete(&id).unwrap();
        client.push(w).await.unwrap();
    }

    fn backup(client: &HavenClient) -> Vec<u8> {
        let payload = export::render(&client.vault().unwrap(), ExportFormat::Backup, 1).unwrap();
        seal_backup(&payload.bytes, &SecretString::from(BACKUP_PW), &fast_kdf()).unwrap()
    }

    fn open(file: &[u8]) -> OpenedBackup {
        open_backup(file, &SecretString::from(BACKUP_PW)).unwrap()
    }

    fn password_of(client: &HavenClient, id: Uuid) -> String {
        let pw = client
            .vault()
            .unwrap()
            .reveal(&id, SecretField::Password)
            .unwrap();
        pw.expose().to_owned()
    }

    #[tokio::test]
    async fn a_deleted_item_comes_back_and_a_second_restore_adds_nothing() {
        let (_dir, client, server) = online().await;
        let gone = create(&client, login("Bank", "pw-bank")).await;
        let kept = create(&client, login("Mail", "pw-mail")).await;
        let file = backup(&client);
        delete(&client, gone).await;
        assert!(
            server.items.lock().unwrap().rows[&gone].blobs.is_none(),
            "a tombstone"
        );

        let report = client.restore_backup(open(&file)).await.unwrap();
        // The Identity and "Mail" are still here; "Bank" is revived.
        assert_eq!(
            (report.imported, report.logins, report.skipped_existing),
            (1, 1, 2)
        );
        assert_eq!(password_of(&client, gone), "pw-bank");
        assert!(server.items.lock().unwrap().rows[&gone].blobs.is_some());

        // Edit the restored item; a second restore must not undo that.
        let edit = login("Bank", "pw-edited");
        let w = client
            .vault()
            .unwrap()
            .stage_update(&gone, edit, 2)
            .unwrap();
        client.push(w).await.unwrap();
        let again = client.restore_backup(open(&file)).await.unwrap();
        assert_eq!(
            (again.imported, again.skipped_existing, again.failed),
            (0, 3, 0)
        );
        assert_eq!(password_of(&client, gone), "pw-edited");
        assert_eq!(password_of(&client, kept), "pw-mail");
    }

    /// Another device writes an item between this device's pull and its
    /// push: that one is skipped, never overwritten, and the rest of the
    /// batch (a tombstone to revive among them) still goes through.
    #[tokio::test]
    async fn a_live_conflict_is_skipped_without_sinking_the_batch() {
        let (_dir, client, server) = online().await;
        let a = create(&client, login("A", "pw-a")).await;
        let b = create(&client, login("B", "pw-b")).await;
        let c = create(&client, login("C", "pw-c")).await;
        let file = backup(&client);
        for id in [a, b, c] {
            delete(&client, id).await;
        }
        client.sync_now().await.unwrap();
        let staged = client
            .vault()
            .unwrap()
            .stage_restore(open(&file), 3)
            .unwrap();
        assert_eq!(staged.writes.len(), 3);

        // Behind this device's back, "B" comes back live elsewhere.
        let theirs = {
            let mut items = server.items.lock().unwrap();
            items.revision += 1;
            let row = StubRow {
                revision: items.revision,
                blobs: Some(("b3Y=".into(), "ZGV0".into())),
            };
            items.rows.insert(b, row.clone());
            row
        };

        let report = client.push_restore(staged).await.unwrap();
        assert_eq!(
            (report.imported, report.logins, report.skipped_existing),
            (2, 2, 2) // + the Identity, skipped while staging
        );
        let items = server.items.lock().unwrap();
        assert_eq!(items.rows[&b].blobs, theirs.blobs, "theirs is untouched");
        assert!(items.rows[&a].blobs.is_some() && items.rows[&c].blobs.is_some());
        drop(items);
        assert_eq!(password_of(&client, a), "pw-a");
        assert_eq!(password_of(&client, c), "pw-c");
    }
}
