//! Sync through a folder the user already syncs (OneDrive, Dropbox, Google
//! Drive, Syncthing…). No server. Format: docs/sync.md.
//!
//! ```text
//! <chosen folder>/HavenKeys/<vault id>/
//!     header.json              {"header": {...unlock header...}, "attestation": "..."}
//!     devices/<device id>.hks  one encrypted snapshot per device
//! ```
//!
//! Each device writes only its own snapshot, so the sync service never sees
//! two devices editing the same file. Each device reads everyone else's and
//! merges item by item: the newest `updated_at` wins, and deletions travel
//! as tombstones.
//!
//! Security:
//! * The folder is untrusted storage. Every snapshot is AES-256-GCM sealed
//!   under the vault's data key and bound to the vault and the writing
//!   device. The items inside are the same per-item blobs as on disk, bound
//!   to their own item IDs. Anything that does not authenticate is ignored.
//! * `header.json` must be plaintext (a new device needs it to unlock), but
//!   it carries an attestation sealed with the data key. A device only adopts
//!   a header that a vault-key holder wrote and that is newer (higher
//!   revision). It never adopts a password-only header.
//! * Sync requires key scheme 2: the copy in the cloud cannot be unlocked
//!   with the master password alone.
//! * Replaying old files cannot roll items back: older versions lose the
//!   merge. Deleting files only stops updates (see docs/sync.md, Limitations).
//!
//! File I/O is kept apart from the merge ([`folder`]) so the caller can read
//! and write a slow cloud folder without holding the vault lock.

use crate::account::AccountRef;
use crate::crypto::blob::{self, BlobContext, Purpose};
use crate::crypto::kdf::{derive_master_key, KdfParams};
use crate::crypto::keys::{derive_auth_key_from_master, derive_data_key, derive_kek_v3, AuthKey};
use crate::crypto::secret_key::SecretKey;
use crate::error::{Error, Result};
use crate::model::{ItemDetails, ItemOverview};
use crate::secret::SecretString;
use crate::store::{AccountRecord, HeaderRecord, ItemRow, KeyScheme};
use crate::vault::{
    derive_kek_for, open_json, seal_json, unwrap_vault_key, PreparedVault, VaultService,
    FORMAT_VERSION,
};
use data_encoding::BASE64;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub const SYNC_FORMAT: u32 = 1;
/// Device snapshots read per sync. Personal use means a handful of devices.
pub const MAX_DEVICES: usize = 32;
/// Largest file read from the folder (a snapshot is one blob).
pub const MAX_FILE_BYTES: usize = blob::MAX_BLOB_LEN;

// ------------------------------------------------------------------ formats

/// Fields of `header.json` covered by the attestation.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HeaderBody {
    format: u32,
    vault_id: Uuid,
    vault_format: u32,
    key_scheme: KeyScheme,
    kdf: KdfParams,
    wrapped_vault_key: String,
    revision: u64,
    created_at: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HeaderFile {
    #[serde(rename = "header")]
    body: HeaderBody,
    /// `seal(data key, sync-header, canonical JSON of body)`, Base64.
    attestation: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Snapshot {
    format: u32,
    device_id: Uuid,
    written_at: i64,
    items: Vec<SnapshotItem>,
    tombstones: Vec<Tombstone>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SnapshotItem {
    id: Uuid,
    /// The item's overview blob, Base64 (still encrypted).
    overview: String,
    /// The item's details blob, Base64 (still encrypted).
    details: String,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Tombstone {
    id: Uuid,
    deleted_at: i64,
}

/// What the caller read from the folder.
#[derive(Default)]
pub struct SyncInput {
    /// `header.json`, if present.
    pub header: Option<Vec<u8>>,
    /// Other devices' snapshots: `(device id from the file name, bytes)`.
    pub devices: Vec<(Uuid, Vec<u8>)>,
}

/// What the caller must write back.
pub struct SyncOutput {
    /// A new `header.json`, when the folder's is missing, invalid or older.
    pub header: Option<Vec<u8>>,
    /// This device's snapshot.
    pub snapshot: Vec<u8>,
    pub report: SyncReport,
}

/// Counts only; never item data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
    /// Snapshots that did not authenticate or parse.
    pub unreadable_devices: usize,
    /// Items inside readable snapshots that did not authenticate.
    pub skipped_items: usize,
    /// A newer header (master password changed on another device) was taken.
    pub header_adopted: bool,
    /// The folder's header did not authenticate and was replaced.
    pub header_rejected: bool,
}

// ------------------------------------------------------------------ header

fn header_body(h: &HeaderRecord) -> HeaderBody {
    HeaderBody {
        format: SYNC_FORMAT,
        vault_id: h.vault_id,
        vault_format: h.format_version,
        key_scheme: h.key_scheme,
        kdf: h.kdf.clone(),
        wrapped_vault_key: BASE64.encode(&h.wrapped_vault_key),
        revision: h.revision,
        created_at: h.created_at,
    }
}

fn body_to_record(b: &HeaderBody) -> Result<HeaderRecord> {
    Ok(HeaderRecord {
        format_version: b.vault_format,
        vault_id: b.vault_id,
        kdf: b.kdf.clone(),
        wrapped_vault_key: BASE64
            .decode(b.wrapped_vault_key.as_bytes())
            .map_err(|_| Error::Corrupted)?,
        created_at: b.created_at,
        key_scheme: b.key_scheme,
        revision: b.revision,
    })
}

fn canonical(body: &HeaderBody) -> Result<Vec<u8>> {
    serde_json::to_vec(body).map_err(|_| Error::Encryption)
}

fn header_ctx(vault_id: Uuid) -> BlobContext {
    BlobContext::vault(Purpose::SyncHeader, vault_id)
}

fn encode_header(data_key: &crate::crypto::keys::Key256, h: &HeaderRecord) -> Result<Vec<u8>> {
    let body = header_body(h);
    let attestation = blob::seal(data_key, &header_ctx(h.vault_id), &canonical(&body)?)?;
    serde_json::to_vec_pretty(&HeaderFile {
        body,
        attestation: BASE64.encode(&attestation),
    })
    .map_err(|_| Error::Encryption)
}

/// Parse `header.json` without verifying it (a new device has no keys yet).
fn parse_header(bytes: &[u8]) -> Result<HeaderFile> {
    if bytes.len() > 64 * 1024 {
        return Err(Error::Corrupted);
    }
    let h: HeaderFile = serde_json::from_slice(bytes).map_err(|_| Error::Corrupted)?;
    if h.body.format != SYNC_FORMAT || h.body.vault_format != FORMAT_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    h.body.kdf.validate()?;
    Ok(h)
}

/// Does the attestation prove a vault-key holder wrote this header?
fn verify_header(data_key: &crate::crypto::keys::Key256, h: &HeaderFile) -> bool {
    let Ok(att) = BASE64.decode(h.attestation.as_bytes()) else {
        return false;
    };
    let Ok(expected) = canonical(&h.body) else {
        return false;
    };
    blob::open(data_key, &header_ctx(h.body.vault_id), &att)
        .map(|plain| plain.as_slice() == expected.as_slice())
        .unwrap_or(false)
}

/// The vault ID a sync folder's `header.json` names (no verification).
pub fn header_vault_id(bytes: &[u8]) -> Result<Uuid> {
    Ok(parse_header(bytes)?.body.vault_id)
}

/// Join a vault from a sync folder on a new device: derive the key from the
/// master password and Secret Key, unwrap the vault key, and check the
/// header's attestation. Slow (Argon2id). Pass the result to
/// [`VaultService::create_vault`], then run [`VaultService::sync`].
pub fn prepare_join(
    header: &[u8],
    password: &SecretString,
    secret_key: &SecretKey,
) -> Result<PreparedVault> {
    let file = parse_header(header)?;
    if file.body.key_scheme != KeyScheme::PasswordAndSecretKey {
        return Err(Error::UnsupportedVersion);
    }
    let record = body_to_record(&file.body)?;
    let kek = derive_kek_for(
        record.key_scheme,
        password,
        &record.kdf,
        &record.vault_id,
        Some(secret_key),
        None,
    )?;
    let vault_key = unwrap_vault_key(&kek, record.vault_id, &record.wrapped_vault_key)?;
    if !verify_header(&derive_data_key(&vault_key)?, &file) {
        return Err(Error::Corrupted);
    }
    Ok(PreparedVault {
        header: record,
        vault_key,
    })
}

/// Sign in to an account vault on a new device: derive the KEK from the
/// master password, the Secret Key and the account, unwrap the vault key,
/// and check the header's attestation. Refuses any key scheme below 3, so a
/// server cannot downgrade a device. Slow (Argon2id).
pub fn prepare_sign_in(
    header: &[u8],
    password: &SecretString,
    secret_key: &SecretKey,
    account: &AccountRef,
) -> Result<(PreparedVault, AuthKey)> {
    let file = parse_header(header)?;
    if file.body.key_scheme != KeyScheme::AccountBound {
        return Err(Error::UnsupportedVersion);
    }
    let record = body_to_record(&file.body)?;
    let master_key = derive_master_key(password, &record.kdf)?;
    let kek = derive_kek_v3(&master_key, secret_key, account)?;
    let auth_key = derive_auth_key_from_master(&master_key, secret_key, account)?;
    let vault_key = unwrap_vault_key(&kek, record.vault_id, &record.wrapped_vault_key)?;
    if !verify_header(&derive_data_key(&vault_key)?, &file) {
        return Err(Error::Corrupted);
    }
    Ok((
        PreparedVault {
            header: record,
            vault_key,
        },
        auth_key,
    ))
}

/// One item's state as it travels to or from the server. Blobs are the same
/// per-item ciphertexts stored locally, copied without re-encryption, so the
/// server never holds anything it could open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteChange {
    pub item_id: Uuid,
    /// `None` for a deletion.
    pub overview: Option<Vec<u8>>,
    /// `None` for a deletion.
    pub details: Option<Vec<u8>>,
    /// `Some` for a deletion, unix milliseconds.
    pub deleted_at: Option<i64>,
}

/// What this device still owes the server, and the cursor those changes were
/// computed against.
#[derive(Clone, Debug)]
pub struct PendingPush {
    pub base_cursor: i64,
    pub changes: Vec<RemoteChange>,
}

impl VaultService {
    /// The header this device would publish to its account's server. Requires
    /// an unlocked key scheme 3 vault.
    pub fn encode_account_header(&self) -> Result<Vec<u8>> {
        let local = self.store.header()?.ok_or(Error::NoVault)?;
        if local.key_scheme != KeyScheme::AccountBound {
            return Err(Error::InvalidInput(
                "this vault is not linked to an account",
            ));
        }
        encode_header(&self.session()?.data_key, &local)
    }

    /// Consider a header served by the account's server. Returns whether the
    /// local header was replaced.
    ///
    /// The server is untrusted, so three things must hold before adoption:
    /// the attestation must verify under this vault's data key (only a vault
    /// key holder could have written it), the key scheme must not go
    /// backwards, and the revision must not go backwards — a genuine old
    /// header replayed after a master-password change would otherwise make
    /// the previous password work again.
    pub fn adopt_account_header(&mut self, remote: &[u8]) -> Result<bool> {
        let local = self.store.header()?.ok_or(Error::NoVault)?;
        let file = parse_header(remote)?;
        if file.body.vault_id != local.vault_id {
            return Err(Error::InvalidInput("that header is for a different vault"));
        }
        if file.body.key_scheme != KeyScheme::AccountBound {
            return Ok(false);
        }
        if !verify_header(&self.session()?.data_key, &file) {
            return Ok(false);
        }
        let record = body_to_record(&file.body)?;
        let floor = self
            .store
            .account()?
            .map(|a| a.max_header_rev)
            .unwrap_or(0)
            .max(local.revision as i64);
        let remote_rev = record.revision as i64;
        if remote_rev <= floor {
            return Ok(false);
        }
        self.store.update_key_wrap(
            &record.kdf,
            &record.wrapped_vault_key,
            record.key_scheme,
            record.revision,
        )?;
        self.store.raise_max_header_rev(remote_rev)?;
        Ok(true)
    }

    /// Store the account this vault belongs to.
    pub fn store_account(&mut self, rec: &AccountRecord) -> Result<()> {
        self.store.set_account(rec)
    }

    /// The account record, or an error when this vault is not linked to one.
    fn require_account(&self) -> Result<AccountRecord> {
        self.store.account()?.ok_or(Error::InvalidInput(
            "this vault is not linked to an account",
        ))
    }

    /// Local changes not yet accepted by the server.
    ///
    /// This reads only opaque blobs already encrypted on disk and the plain
    /// account/dirty bookkeeping, so — unlike most of `VaultService` — it
    /// does not require the vault to be unlocked. `confirm_push` is the
    /// same: no key material is needed to mark a push acknowledged.
    pub fn pending_push(&self) -> Result<PendingPush> {
        let base_cursor = self.require_account()?.server_cursor;
        let mut changes: Vec<RemoteChange> = self
            .store
            .dirty_rows()?
            .into_iter()
            .map(|(item_id, overview, details)| RemoteChange {
                item_id,
                overview: Some(overview),
                details: Some(details),
                deleted_at: None,
            })
            .collect();
        changes.extend(
            self.store
                .dirty_tombstones()?
                .into_iter()
                .map(|(item_id, deleted_at)| RemoteChange {
                    item_id,
                    overview: None,
                    details: None,
                    deleted_at: Some(deleted_at),
                }),
        );
        Ok(PendingPush {
            base_cursor,
            changes,
        })
    }

    /// Merge a delta from the server and advance the cursor.
    ///
    /// The server is untrusted: every non-deletion must authenticate under
    /// this vault's data key and match its own item ID, or it is skipped and
    /// counted. Which version wins is decided on decrypted content by the
    /// same rules the folder path used (docs/sync.md §5) — the cursor only
    /// says what to fetch.
    pub fn apply_remote_changes(
        &mut self,
        cursor: i64,
        changes: Vec<RemoteChange>,
        now_ms: i64,
    ) -> Result<SyncReport> {
        self.require_account()?;
        let mut report = SyncReport::default();
        let vault_id = self.session()?.vault_id;
        let mut candidates: HashMap<Uuid, Candidate> = HashMap::new();
        let mut remote_tombs: HashMap<Uuid, i64> = HashMap::new();

        for change in changes {
            match (change.deleted_at, change.overview, change.details) {
                (Some(at), _, _) => {
                    let e = remote_tombs.entry(change.item_id).or_insert(at);
                    *e = (*e).max(at);
                }
                (None, Some(ov), Some(det)) => {
                    match self.check_item_bytes(vault_id, change.item_id, ov, det) {
                        Some((row, overview)) => {
                            // A batch can carry more than one version of the
                            // same item (e.g. the server replays a range);
                            // keep the newest by content, same as the
                            // snapshot path below, not whichever came last.
                            let better =
                                candidates.get(&change.item_id).is_none_or(|c: &Candidate| {
                                    overview.updated_at > c.overview.updated_at
                                });
                            if better {
                                candidates.insert(
                                    change.item_id,
                                    Candidate {
                                        row,
                                        overview,
                                        device: Uuid::nil(),
                                    },
                                );
                            }
                        }
                        None => report.skipped_items += 1,
                    }
                }
                (None, _, _) => report.skipped_items += 1,
            }
        }

        self.decide_merge(candidates, remote_tombs, &mut report)?;
        self.store.set_cursor(cursor, now_ms)?;
        Ok(report)
    }

    /// The server accepted exactly these changes at `cursor`.
    ///
    /// Takes the same `RemoteChange`s `pending_push` returned (the caller
    /// already holds them) rather than bare IDs, so the dirty flag clears
    /// only for the versions actually pushed. A row edited, or a tombstone
    /// re-dated, after `pending_push` read it and before this call arrives
    /// keeps its dirty flag and is reported by the next `pending_push`.
    pub fn confirm_push(
        &mut self,
        cursor: i64,
        pushed: &[RemoteChange],
        now_ms: i64,
    ) -> Result<()> {
        self.require_account()?;
        let mut items: Vec<ItemRow> = Vec::new();
        let mut tombstones: Vec<(Uuid, i64)> = Vec::new();
        for change in pushed {
            match (&change.overview, &change.details, change.deleted_at) {
                (Some(ov), Some(det), None) => {
                    items.push((change.item_id, ov.clone(), det.clone()))
                }
                (None, None, Some(at)) => tombstones.push((change.item_id, at)),
                // Malformed shapes cannot match any dirty row; nothing to clear.
                _ => {}
            }
        }
        self.store.clear_dirty(&items, &tombstones)?;
        self.store.set_cursor(cursor, now_ms)
    }
}

// ------------------------------------------------------------------ snapshots

fn snapshot_ctx(vault_id: Uuid, device_id: Uuid) -> BlobContext {
    BlobContext::item(Purpose::SyncSnapshot, vault_id, device_id)
}

/// A remote item that authenticated, with the time that decides the merge.
struct Candidate {
    row: ItemRow,
    overview: ItemOverview,
    device: Uuid,
}

impl VaultService {
    /// Merge other devices' snapshots into this vault and produce this
    /// device's snapshot (and header, if the folder needs one). Requires an
    /// unlocked key-scheme-2 vault. No file I/O; see [`folder`].
    pub fn sync(&mut self, device_id: Uuid, input: SyncInput, now_ms: i64) -> Result<SyncOutput> {
        let mut report = SyncReport::default();
        let local = self.store.header()?.ok_or(Error::NoVault)?;
        if local.key_scheme != KeyScheme::PasswordAndSecretKey {
            return Err(Error::InvalidInput("set up a Secret Key before syncing"));
        }
        let header_out = self.sync_header(&local, input.header.as_deref(), &mut report)?;

        let (vault_id, candidates, remote_tombs) = {
            let session = self.session()?;
            let vault_id = session.vault_id;
            let mut candidates: HashMap<Uuid, Candidate> = HashMap::new();
            let mut tombs: HashMap<Uuid, i64> = HashMap::new();
            for (dev, bytes) in input.devices.iter().take(MAX_DEVICES) {
                if *dev == device_id {
                    continue;
                }
                let Some(snap) = self.open_snapshot(vault_id, *dev, bytes) else {
                    report.unreadable_devices += 1;
                    continue;
                };
                for item in snap.items {
                    match self.check_item(vault_id, &item) {
                        Some((row, overview)) => {
                            let better = candidates.get(&item.id).is_none_or(|c| {
                                (overview.updated_at, *dev) > (c.overview.updated_at, c.device)
                            });
                            if better {
                                candidates.insert(
                                    item.id,
                                    Candidate {
                                        row,
                                        overview,
                                        device: *dev,
                                    },
                                );
                            }
                        }
                        None => report.skipped_items += 1,
                    }
                }
                for t in snap.tombstones {
                    let e = tombs.entry(t.id).or_insert(t.deleted_at);
                    *e = (*e).max(t.deleted_at);
                }
            }
            (vault_id, candidates, tombs)
        };

        self.decide_merge(candidates, remote_tombs, &mut report)?;

        let snapshot = self.encode_snapshot(vault_id, device_id, now_ms)?;
        Ok(SyncOutput {
            header: header_out,
            snapshot,
            report,
        })
    }

    /// Apply the merge rules (docs/sync.md §5) to one set of remote
    /// candidates and tombstones. The only place these rules exist.
    fn decide_merge(
        &mut self,
        candidates: HashMap<Uuid, Candidate>,
        remote_tombs: HashMap<Uuid, i64>,
        report: &mut SyncReport,
    ) -> Result<()> {
        let local_tombs: HashMap<Uuid, i64> = self.store.tombstones()?.into_iter().collect();
        let mut upserts: Vec<ItemRow> = Vec::new();
        let mut new_overviews: Vec<ItemOverview> = Vec::new();
        let mut resurrected: Vec<Uuid> = Vec::new();
        let mut deletions: Vec<(Uuid, i64)> = Vec::new();
        {
            let session = self.session()?;
            for (id, c) in candidates {
                let ru = c.overview.updated_at;
                // A newer deletion elsewhere beats this version.
                if remote_tombs.get(&id).is_some_and(|&rt| rt >= ru) {
                    continue;
                }
                match (session.overviews.get(&id), local_tombs.get(&id)) {
                    (Some(l), _) if ru > l.updated_at => report.updated += 1,
                    (Some(_), _) => continue,
                    (None, Some(&lt)) if ru > lt => {
                        resurrected.push(id);
                        report.added += 1;
                    }
                    (None, Some(_)) => continue,
                    (None, None) => report.added += 1,
                }
                upserts.push(c.row);
                new_overviews.push(c.overview);
            }
            for (&id, &rt) in &remote_tombs {
                if upserts.iter().any(|(u, _, _)| *u == id) {
                    continue;
                }
                match (session.overviews.get(&id), local_tombs.get(&id)) {
                    (Some(l), _) if rt >= l.updated_at => {
                        deletions.push((id, rt));
                        report.deleted += 1;
                    }
                    (Some(_), _) => {}
                    (None, Some(&lt)) if rt <= lt => {}
                    (None, _) => deletions.push((id, rt)),
                }
            }
        }

        if !upserts.is_empty() || !deletions.is_empty() {
            self.store.apply_merge(&upserts, &deletions, &resurrected)?;
            let session = self.session_mut()?;
            for ov in new_overviews {
                session.overviews.insert(ov.id, ov);
            }
            for (id, _) in &deletions {
                session.overviews.remove(id);
            }
        }

        Ok(())
    }

    /// Decide about `header.json`. Returns the bytes to write, if any.
    fn sync_header(
        &mut self,
        local: &HeaderRecord,
        remote: Option<&[u8]>,
        report: &mut SyncReport,
    ) -> Result<Option<Vec<u8>>> {
        let data_key = &self.session()?.data_key;
        let ours = encode_header(data_key, local)?;
        let Some(bytes) = remote else {
            return Ok(Some(ours));
        };
        let parsed = match parse_header(bytes) {
            Ok(p) if p.body.vault_id != local.vault_id => {
                return Err(Error::InvalidInput(
                    "the sync folder holds a different vault",
                ))
            }
            Ok(p) => p,
            Err(_) => {
                report.header_rejected = true;
                return Ok(Some(ours));
            }
        };
        if !verify_header(data_key, &parsed)
            || parsed.body.key_scheme != KeyScheme::PasswordAndSecretKey
        {
            report.header_rejected = true;
            return Ok(Some(ours));
        }
        let remote_record = body_to_record(&parsed.body)?;
        let newer = (remote_record.revision, &remote_record.wrapped_vault_key)
            > (local.revision, &local.wrapped_vault_key);
        if newer {
            // The master password changed on another device. Take its
            // header; this device's next unlock needs the new password.
            self.store.update_key_wrap(
                &remote_record.kdf,
                &remote_record.wrapped_vault_key,
                remote_record.key_scheme,
                remote_record.revision,
            )?;
            report.header_adopted = true;
            Ok(None)
        } else if remote_record.revision == local.revision
            && remote_record.wrapped_vault_key == local.wrapped_vault_key
        {
            Ok(None)
        } else {
            Ok(Some(ours))
        }
    }

    fn open_snapshot(&self, vault_id: Uuid, device: Uuid, bytes: &[u8]) -> Option<Snapshot> {
        if bytes.len() > MAX_FILE_BYTES {
            return None;
        }
        let data_key = &self.session().ok()?.data_key;
        let snap: Snapshot = open_json(data_key, &snapshot_ctx(vault_id, device), bytes).ok()?;
        (snap.format == SYNC_FORMAT && snap.device_id == device).then_some(snap)
    }

    /// Authenticate one remote item (overview and details) and return its row.
    fn check_item(&self, vault_id: Uuid, item: &SnapshotItem) -> Option<(ItemRow, ItemOverview)> {
        let ov_blob = BASE64.decode(item.overview.as_bytes()).ok()?;
        let det_blob = BASE64.decode(item.details.as_bytes()).ok()?;
        self.check_item_bytes(vault_id, item.id, ov_blob, det_blob)
    }

    /// Authenticate one remote item version. Returns `None` unless both blobs
    /// open under this vault's data key, the overview's ID matches the row's,
    /// and the details type matches the overview type.
    fn check_item_bytes(
        &self,
        vault_id: Uuid,
        id: Uuid,
        ov_blob: Vec<u8>,
        det_blob: Vec<u8>,
    ) -> Option<(ItemRow, ItemOverview)> {
        let data_key = &self.session().ok()?.data_key;
        let ov: ItemOverview = open_json(
            data_key,
            &BlobContext::item(Purpose::ItemOverview, vault_id, id),
            &ov_blob,
        )
        .ok()?;
        let details: ItemDetails = open_json(
            data_key,
            &BlobContext::item(Purpose::ItemDetails, vault_id, id),
            &det_blob,
        )
        .ok()?;
        if ov.id != id || details.item_type() != ov.item_type {
            return None;
        }
        Some(((id, ov_blob, det_blob), ov))
    }

    fn encode_snapshot(&self, vault_id: Uuid, device_id: Uuid, now_ms: i64) -> Result<Vec<u8>> {
        let items = self
            .store
            .item_rows()?
            .into_iter()
            .map(|(id, ov, det)| SnapshotItem {
                id,
                overview: BASE64.encode(&ov),
                details: BASE64.encode(&det),
            })
            .collect();
        let tombstones = self
            .store
            .tombstones()?
            .into_iter()
            .map(|(id, deleted_at)| Tombstone { id, deleted_at })
            .collect();
        let snap = Snapshot {
            format: SYNC_FORMAT,
            device_id,
            written_at: now_ms,
            items,
            tombstones,
        };
        seal_json(
            &self.session()?.data_key,
            &snapshot_ctx(vault_id, device_id),
            &snap,
        )
    }
}

// ------------------------------------------------------------------ folder I/O

/// Reading and writing the sync folder. No keys, no vault lock needed.
pub mod folder {
    use super::{SyncInput, MAX_DEVICES, MAX_FILE_BYTES};
    use std::fs;
    use std::io::{self, Read, Write};
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    const APP_DIR: &str = "HavenKeys";
    const HEADER: &str = "header.json";
    const DEVICES: &str = "devices";
    const EXT: &str = "hks";

    /// `<root>/HavenKeys/<vault id>`.
    pub fn vault_dir(root: &Path, vault_id: Uuid) -> PathBuf {
        root.join(APP_DIR).join(vault_id.to_string())
    }

    fn read_capped(path: &Path) -> io::Result<Vec<u8>> {
        let f = fs::File::open(path)?;
        let mut buf = Vec::new();
        f.take(MAX_FILE_BYTES as u64 + 1).read_to_end(&mut buf)?;
        if buf.len() > MAX_FILE_BYTES {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "file too large"));
        }
        Ok(buf)
    }

    /// Vaults present under a chosen folder (for joining from a new device).
    pub fn list_vaults(root: &Path) -> Vec<Uuid> {
        let Ok(entries) = fs::read_dir(root.join(APP_DIR)) else {
            return Vec::new();
        };
        let mut out: Vec<Uuid> = entries
            .flatten()
            .filter_map(|e| Uuid::parse_str(&e.file_name().to_string_lossy()).ok())
            .filter(|id| vault_dir(root, *id).join(HEADER).is_file())
            .take(MAX_DEVICES)
            .collect();
        out.sort();
        out
    }

    pub fn read_header(dir: &Path) -> io::Result<Vec<u8>> {
        read_capped(&dir.join(HEADER))
    }

    /// Read the header and every other device's snapshot. Files that are
    /// not `<uuid>.hks`, are too big, or cannot be read are skipped.
    pub fn read(dir: &Path, me: Uuid) -> io::Result<SyncInput> {
        let header = match read_capped(&dir.join(HEADER)) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        let mut devices = Vec::new();
        if let Ok(entries) = fs::read_dir(dir.join(DEVICES)) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some(EXT) {
                    continue;
                }
                let Some(id) = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
                else {
                    continue;
                };
                if id == me || devices.len() >= MAX_DEVICES {
                    continue;
                }
                if let Ok(bytes) = read_capped(&path) {
                    devices.push((id, bytes));
                }
            }
        }
        Ok(SyncInput { header, devices })
    }

    /// Write a file so readers never see it half-written: a temporary file
    /// in the same folder, flushed, then renamed over the target.
    fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let dir = path.parent().ok_or_else(|| io::Error::other("no parent"))?;
        fs::create_dir_all(dir)?;
        let tmp = dir.join(format!(
            ".{}.tmp",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
        ));
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, path)
    }

    pub fn write(dir: &Path, me: Uuid, header: Option<&[u8]>, snapshot: &[u8]) -> io::Result<()> {
        if let Some(h) = header {
            write_atomic(&dir.join(HEADER), h)?;
        }
        write_atomic(&dir.join(DEVICES).join(format!("{me}.{EXT}")), snapshot)
    }
}
