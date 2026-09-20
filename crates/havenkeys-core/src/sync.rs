//! The account header (docs/crypto.md) and applying a server pull.
//!
//! * `encode_account_header`/`adopt_account_header`/`prepare_sign_in` handle
//!   the plaintext-but-attested `header.json` a device publishes to and reads
//!   from its account's server, so a new device can unlock without the
//!   server ever holding key material. A device only adopts a header that a
//!   vault-key holder wrote (the attestation) and that is newer (the
//!   revision never goes backwards).
//! * `apply_remote_changes` is the local replica catching up to the server:
//!   the server is the single writer, so this is not a merge, just an upsert
//!   or a deletion per item, in cursor order (spec 2026-09-20 §8.5). A later
//!   task owns the server-side transport that feeds it.
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

/// One item as the server serves it. Blobs are the same per-item ciphertexts
/// stored locally, copied without re-encryption, so the server never holds
/// anything it could open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteChange {
    pub item_id: Uuid,
    pub revision: i64,
    /// `None` for a deletion.
    pub overview: Option<Vec<u8>>,
    /// `None` for a deletion.
    pub details: Option<Vec<u8>>,
    pub deleted: bool,
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

    /// Apply a pull in cursor order and advance the stored cursor.
    ///
    /// This is not a merge: the server is the single writer, so a change is
    /// an upsert or a deletion. The only judgement is structural: a blob
    /// that does not open under this vault's data key, or that carries
    /// another item's id, is counted in `skipped_items` and leaves the
    /// existing row untouched — a hostile server can fail to update the
    /// replica, not corrupt it.
    ///
    /// A batch may mention the same item id more than once (edited, then
    /// deleted, since the cursor this device last saw — or the other way
    /// around). Only the *last* change for a given item, by its position in
    /// `changes`, is applied; every earlier change for that same item is
    /// superseded and dropped as if it had never been pulled. Applying them
    /// independently (or in an order that does not match how the server
    /// produced them) is exactly the bug this guards against: it can leave
    /// the store and the session cache disagreeing about whether the item
    /// exists. `added`/`updated`/`deleted` therefore count the batch's net
    /// effect on each item, once, not once per change that touched it.
    pub fn apply_remote_changes(
        &mut self,
        cursor: i64,
        changes: Vec<RemoteChange>,
        now_ms: i64,
    ) -> Result<SyncReport> {
        self.require_account()?;
        let vault_id = self.session()?.vault_id;
        let mut report = SyncReport::default();
        let mut rows: Vec<(Uuid, Vec<u8>, Vec<u8>, i64)> = Vec::new();
        let mut overviews: Vec<ItemOverview> = Vec::new();
        let mut deletions: Vec<Uuid> = Vec::new();

        let mut last_index: HashMap<Uuid, usize> = HashMap::new();
        for (index, change) in changes.iter().enumerate() {
            last_index.insert(change.item_id, index);
        }

        for (index, change) in changes.into_iter().enumerate() {
            if last_index.get(&change.item_id) != Some(&index) {
                // A later change in this same batch supersedes this one.
                continue;
            }
            if change.deleted {
                deletions.push(change.item_id);
                continue;
            }
            let (Some(ov), Some(det)) = (change.overview, change.details) else {
                report.skipped_items += 1;
                continue;
            };
            match self.check_item_bytes(vault_id, change.item_id, ov, det) {
                Some(((id, ov, det), overview)) => {
                    if self.session()?.overviews.contains_key(&id) {
                        report.updated += 1;
                    } else {
                        report.added += 1;
                    }
                    rows.push((id, ov, det, change.revision));
                    overviews.push(overview);
                }
                None => report.skipped_items += 1,
            }
        }

        // Every id above is the *last* change for that item, so an id
        // appears in at most one of `rows` (via `overviews`) and
        // `deletions` — never both. The two loops below can run in either
        // order without one clobbering the other's result.
        self.store.upsert_items(&rows)?;
        for overview in overviews {
            self.session_mut()?.overviews.insert(overview.id, overview);
        }
        for id in &deletions {
            if self.store.delete_item(id)? {
                report.deleted += 1;
            }
            self.session_mut()?.overviews.remove(id);
        }
        self.store.set_cursor(cursor, now_ms)?;
        Ok(report)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::NormalizedEmail;
    use crate::crypto::kdf::test_params;
    use crate::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
    use crate::store::{AccountRecord, Store};
    use crate::vault::prepare_new_account_vault;

    const PASSWORD: &str = "correct horse battery staple";
    const NOW: i64 = 1_700_000_000_000;

    fn account() -> AccountRef {
        AccountRef::new(
            Uuid::from_u128(0x5eed),
            NormalizedEmail::parse("user@example.com").unwrap(),
        )
    }

    fn login(title: &str) -> ItemInput {
        ItemInput {
            item_type: ItemType::Login,
            title: title.into(),
            username: Some("me".into()),
            urls: vec![UrlRule {
                url: "github.com".into(),
                match_type: MatchType::Domain,
            }],
            password: SecretUpdate::Set(SecretString::from("pw")),
            totp: SecretUpdate::Keep,
            notes: SecretUpdate::Keep,
            content: SecretUpdate::Keep,
        }
    }

    /// An activated, unlocked account vault, matching what a real device
    /// building `apply_remote_changes` requests looks like.
    fn activated_vault() -> VaultService {
        let made = prepare_new_account_vault(
            &SecretString::from(PASSWORD),
            &account(),
            test_params(),
            NOW,
        )
        .unwrap();
        let sk = made.secret_key;
        let mut vault = VaultService::new(Store::open_in_memory().unwrap());
        vault
            .create_account_vault(
                made.prepared,
                &AccountRecord {
                    account_id: account().id,
                    email: "user@example.com".into(),
                    server_url: "https://vault.example.com".into(),
                    server_cursor: 0,
                    max_header_rev: 0,
                    last_synced_at: None,
                },
            )
            .unwrap();
        vault.lock();
        vault
            .unlock_for_account(&SecretString::from(PASSWORD), &sk, &account())
            .unwrap();
        vault
    }

    #[test]
    fn apply_remote_changes_requires_a_linked_account() {
        let mut vault = VaultService::new(Store::open_in_memory().unwrap());
        let err = vault.apply_remote_changes(1, vec![], NOW).unwrap_err();
        assert_eq!(err.code(), "invalid_input");
    }

    #[test]
    fn a_remote_change_adds_updates_and_deletes_without_asking_who_is_newer() {
        let mut vault = activated_vault();
        let staged = vault.stage_create(login("GitHub"), NOW).unwrap();
        let item = vault.commit_write(staged, 1).unwrap().unwrap();
        let id = item.id;
        let overview = vault
            .store
            .item_overviews()
            .unwrap()
            .pop()
            .unwrap()
            .unwrap()
            .1;
        let details = vault.store.item_details(&id).unwrap().unwrap();

        // Remove the local copy so the same blobs, replayed as a remote
        // change, exercise the "added" path rather than "updated".
        let staged = vault.stage_delete(&id).unwrap();
        vault.commit_write(staged, 2).unwrap();

        let report = vault
            .apply_remote_changes(
                5,
                vec![RemoteChange {
                    item_id: id,
                    revision: 5,
                    overview: Some(overview.clone()),
                    details: Some(details.clone()),
                    deleted: false,
                }],
                NOW,
            )
            .unwrap();
        assert_eq!(report.added, 1);
        assert_eq!(vault.get_item(&id).unwrap().title, "GitHub");
        assert_eq!(vault.account().unwrap().unwrap().server_cursor, 5);

        // The same item again is an update, not an add.
        let report = vault
            .apply_remote_changes(
                6,
                vec![RemoteChange {
                    item_id: id,
                    revision: 6,
                    overview: Some(overview),
                    details: Some(details),
                    deleted: false,
                }],
                NOW,
            )
            .unwrap();
        assert_eq!(report.updated, 1);

        // No timestamp comparison, no resurrection arm: the server said so.
        let report = vault
            .apply_remote_changes(
                7,
                vec![RemoteChange {
                    item_id: id,
                    revision: 7,
                    overview: None,
                    details: None,
                    deleted: true,
                }],
                NOW,
            )
            .unwrap();
        assert_eq!(report.deleted, 1);
        assert!(vault.get_item(&id).is_err());
    }

    #[test]
    fn a_blob_that_does_not_authenticate_is_skipped_not_applied() {
        let mut vault = activated_vault();
        let report = vault
            .apply_remote_changes(
                1,
                vec![RemoteChange {
                    item_id: Uuid::from_u128(1),
                    revision: 1,
                    overview: Some(vec![0u8; 64]),
                    details: Some(vec![0u8; 64]),
                    deleted: false,
                }],
                NOW,
            )
            .unwrap();
        assert_eq!(report.skipped_items, 1);
        assert_eq!(report.added, 0);
        assert!(vault.list_items().unwrap().is_empty());
    }
}
