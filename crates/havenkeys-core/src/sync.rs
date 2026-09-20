//! The account header (docs/crypto.md) and delta sync against a server
//! cursor.
//!
//! * `encode_account_header`/`adopt_account_header`/`prepare_sign_in` handle
//!   the plaintext-but-attested `header.json` a device publishes to and reads
//!   from its account's server, so a new device can unlock without the
//!   server ever holding key material. A device only adopts a header that a
//!   vault-key holder wrote (the attestation) and that is newer (the
//!   revision never goes backwards).
//! * `pending_push`/`confirm_push`/`apply_remote_changes` are the local half
//!   of delta sync: what this device still owes the server, and how a batch
//!   of the server's changes is merged in. The merge itself
//!   (`decide_merge`/`Candidate`) is shared with `apply_merge` in
//!   `store.rs`; a later task owns the server-side transport and pull
//!   applier around these.
//!
//! Security: every blob here is the same per-item AES-256-GCM ciphertext
//! stored locally, copied without re-encryption, bound to its own item ID.
//! The server is untrusted storage; anything that does not authenticate
//! under this vault's data key is skipped, not applied.

use crate::account::AccountRef;
use crate::crypto::blob::{self, BlobContext, Purpose};
use crate::crypto::kdf::{derive_master_key, KdfParams};
use crate::crypto::keys::{derive_auth_key_from_master, derive_data_key, derive_kek_v3, AuthKey};
use crate::crypto::secret_key::SecretKey;
use crate::error::{Error, Result};
use crate::model::{ItemDetails, ItemOverview};
use crate::secret::SecretString;
use crate::store::{AccountRecord, HeaderRecord, ItemRow, KeyScheme};
use crate::vault::{open_json, unwrap_vault_key, PreparedVault, VaultService, FORMAT_VERSION};
use data_encoding::BASE64;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub const SYNC_FORMAT: u32 = 1;

/// Generous clock-skew allowance for a remote deletion's `deleted_at`
/// (`RemoteChange`) against this device's `now_ms` in
/// [`VaultService::apply_remote_changes`].
///
/// `deleted_at` is plaintext the server supplies and is not authenticated
/// (see docs/crypto.md, "Key scheme 3 (account)"), so this is a partial
/// mitigation, not a fix: it only rejects the worst forgeries, such as
/// `deleted_at = i64::MAX`, which would otherwise poison that item ID
/// forever (the resurrection arm of `decide_merge` could never be satisfied
/// again). A hostile server sending a plausible `deleted_at` — even
/// `now_ms` itself — still deletes the item; that requires authenticated
/// tombstones, which do not exist yet. 24 hours is generous on purpose:
/// legitimate clock skew between two real devices is not zero, and the
/// merge already decides outcomes on wall clocks.
const MAX_FUTURE_DELETION_SKEW_MS: i64 = 24 * 60 * 60 * 1000;

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

/// Counts only; never item data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
    /// Remote items that did not authenticate under this vault's data key.
    pub skipped_items: usize,
    /// A newer header (master password changed on another device) was taken.
    pub header_adopted: bool,
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

/// A remote item that authenticated, with the time that decides the merge.
struct Candidate {
    row: ItemRow,
    overview: ItemOverview,
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
        if local.key_scheme != KeyScheme::AccountBound {
            return Err(Error::InvalidInput(
                "this vault is not linked to an account",
            ));
        }
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
                    // Partial mitigation only (see MAX_FUTURE_DELETION_SKEW_MS):
                    // reject deletions implausibly far in the future rather
                    // than let a forged `deleted_at` poison this item ID
                    // permanently.
                    if at > now_ms.saturating_add(MAX_FUTURE_DELETION_SKEW_MS) {
                        report.skipped_items += 1;
                        continue;
                    }
                    let e = remote_tombs.entry(change.item_id).or_insert(at);
                    *e = (*e).max(at);
                }
                (None, Some(ov), Some(det)) => {
                    match self.check_item_bytes(vault_id, change.item_id, ov, det) {
                        Some((row, overview)) => {
                            // A batch can carry more than one version of the
                            // same item (e.g. the server replays a range);
                            // keep the newest by content, not whichever came
                            // last.
                            let better =
                                candidates.get(&change.item_id).is_none_or(|c: &Candidate| {
                                    overview.updated_at > c.overview.updated_at
                                });
                            if better {
                                candidates.insert(change.item_id, Candidate { row, overview });
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
}
