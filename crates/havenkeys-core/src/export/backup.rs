//! The encrypted HavenKeys backup (`.hkbackup`), spec 2026-10-05-export §5.

use crate::crypto::blob::{self, BlobContext};
use crate::crypto::kdf::{derive_master_key, KdfAlgorithm, KdfParams, SALT_LEN};
use crate::error::{Error, Result};
use crate::import::MAX_ITEMS;
use crate::model::{ItemDetails, ItemOverview};
use crate::secret::SecretString;
use serde::{Deserialize, Serialize};

pub const PAYLOAD_VERSION: u32 = 1;

/// One item exactly as the vault holds it, passkeys included.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupItem {
    pub overview: ItemOverview,
    pub details: ItemDetails,
}

impl std::fmt::Debug for BackupItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BackupItem(<redacted>)")
    }
}

/// The payload envelope as read back. Items stay raw JSON here and are
/// parsed one by one, so a single item this version cannot read is counted
/// as failed instead of sinking the whole restore.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Payload {
    version: u32,
    #[allow(dead_code)]
    exported_at: i64,
    items: Vec<serde_json::Value>,
}

/// What `open_backup` read: the items that parsed, and how many did not.
pub struct OpenedBackup {
    pub items: Vec<BackupItem>,
    /// Items present in the file that this version cannot read; counted as
    /// `failed` by the restore.
    pub unreadable: usize,
}

impl std::fmt::Debug for OpenedBackup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenedBackup")
            .field("items", &self.items.len())
            .field("unreadable", &self.unreadable)
            .finish()
    }
}

/// Serialize borrowed items without cloning their secrets.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PayloadRef<'a> {
    pub version: u32,
    pub exported_at: i64,
    pub items: &'a [BackupItemRef<'a>],
}

#[derive(Serialize)]
pub(crate) struct BackupItemRef<'a> {
    pub overview: &'a ItemOverview,
    pub details: &'a ItemDetails,
}

pub const MAGIC: &[u8; 8] = b"HKBACKUP";
pub const FILE_VERSION: u8 = 1;
const KDF_ARGON2ID: u8 = 1;
/// magic(8) version(1) kdf(1) memory(4) iterations(4) parallelism(4) salt(16)
pub const HEADER_LEN: usize = 8 + 1 + 1 + 4 + 4 + 4 + SALT_LEN;
pub const MAX_BACKUP_BYTES: u64 = crate::import::MAX_TEXT_EXPORT_BYTES;

const NOT_A_BACKUP: Error = Error::InvalidInput("not a HavenKeys backup file");
const NEWER: Error = Error::InvalidInput("this backup was made by a newer version of HavenKeys");
const WRONG_PASSWORD: Error = Error::InvalidInput("wrong backup password, or the file is damaged");
/// The file decrypted, so the password was right and the bytes are what was
/// sealed; this version just cannot read what is inside.
const UNREADABLE: Error =
    Error::InvalidInput("this backup can't be read by this version of HavenKeys");

fn header(kdf: &KdfParams) -> [u8; HEADER_LEN] {
    let mut h = [0u8; HEADER_LEN];
    h[..8].copy_from_slice(MAGIC);
    h[8] = FILE_VERSION;
    h[9] = KDF_ARGON2ID;
    h[10..14].copy_from_slice(&kdf.memory_kib.to_le_bytes());
    h[14..18].copy_from_slice(&kdf.iterations.to_le_bytes());
    h[18..22].copy_from_slice(&kdf.parallelism.to_le_bytes());
    h[22..].copy_from_slice(&kdf.salt);
    h
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Encrypt a payload from `render(.., Backup, ..)` under `password`.
/// Slow (Argon2id): call without holding the vault lock.
pub fn seal_backup(payload: &[u8], password: &SecretString, kdf: &KdfParams) -> Result<Vec<u8>> {
    // Refuse before the slow KDF: a file open_backup would refuse as too large
    // could never be restored.
    if HEADER_LEN
        .saturating_add(blob::MIN_BLOB_LEN)
        .saturating_add(payload.len())
        > MAX_BACKUP_BYTES as usize
    {
        return Err(Error::InvalidInput("backup file is too large"));
    }
    let header = header(kdf);
    let key = derive_master_key(password, kdf)?;
    let sealed = blob::seal(&key, &BlobContext::backup(&header), payload)?;
    let mut out = Vec::with_capacity(HEADER_LEN + sealed.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// Read a backup file: hostile input, checked in the order of spec §5.1.
/// Slow (Argon2id).
pub fn open_backup(file: &[u8], password: &SecretString) -> Result<OpenedBackup> {
    if file.len() as u64 > MAX_BACKUP_BYTES {
        return Err(Error::InvalidInput("backup file is too large"));
    }
    if file.len() <= HEADER_LEN || &file[..8] != MAGIC {
        return Err(NOT_A_BACKUP);
    }
    if file[8] != FILE_VERSION {
        return Err(if file[8] > FILE_VERSION {
            NEWER
        } else {
            NOT_A_BACKUP
        });
    }
    if file[9] != KDF_ARGON2ID {
        return Err(NOT_A_BACKUP);
    }
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&file[22..HEADER_LEN]);
    let kdf = KdfParams {
        algorithm: KdfAlgorithm::Argon2id,
        memory_kib: u32_at(file, 10),
        iterations: u32_at(file, 14),
        parallelism: u32_at(file, 18),
        salt,
    };
    kdf.validate().map_err(|_| NOT_A_BACKUP)?;
    let (head, body) = file.split_at(HEADER_LEN);
    let key = derive_master_key(password, &kdf)?;
    let plain = blob::open(&key, &BlobContext::backup(head), body).map_err(|_| WRONG_PASSWORD)?;
    // From here on the AEAD has authenticated the bytes: a failure is a
    // format this version does not understand, never a wrong password.
    // Version first, so a newer payload says so.
    #[derive(serde::Deserialize)]
    struct Version {
        version: u32,
    }
    let v: Version = serde_json::from_slice(&plain).map_err(|_| UNREADABLE)?;
    if v.version != PAYLOAD_VERSION {
        return Err(if v.version > PAYLOAD_VERSION {
            NEWER
        } else {
            UNREADABLE
        });
    }
    let payload: Payload = serde_json::from_slice(&plain).map_err(|_| UNREADABLE)?;
    if payload.version != PAYLOAD_VERSION {
        return Err(UNREADABLE);
    }
    if payload.items.len() > MAX_ITEMS {
        return Err(Error::InvalidInput("backup contains too many items"));
    }
    let mut items = Vec::with_capacity(payload.items.len());
    let mut unreadable = 0;
    for raw in payload.items {
        match serde_json::from_value::<BackupItem>(raw) {
            Ok(item) => items.push(item),
            Err(_) => unreadable += 1,
        }
    }
    Ok(OpenedBackup { items, unreadable })
}
