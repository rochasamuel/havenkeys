//! The encrypted HavenKeys backup (`.hkbackup`), spec 2026-10-05-export §5;
//! version 2 carries attachments (spec 2026-10-13-encrypted-attachments §4.8).

use crate::attachment::{AttachmentMeta, MAX_ATTACHMENT_BYTES, MAX_ATTACHMENT_VAULT_BYTES};
use crate::crypto::blob::{self, BlobContext};
use crate::crypto::kdf::{derive_master_key, KdfAlgorithm, KdfParams, SALT_LEN};
use crate::crypto::keys::Key256;
use crate::error::{Error, Result};
use crate::import::MAX_ITEMS;
use crate::model::{ItemDetails, ItemOverview};
use crate::secret::{SecretBytes, SecretString};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use uuid::Uuid;
use zeroize::Zeroizing;

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

/// What `open_backup_stream` read: the items that parsed, and how many did not.
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

/// One attached file as a backup holds it. `item_id` is the item's id in the
/// backup; a restore re-seals the bytes under the restored item.
pub struct BackupAttachment {
    pub item_id: Uuid,
    pub meta: AttachmentMeta,
    pub bytes: SecretBytes,
}

impl std::fmt::Debug for BackupAttachment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BackupAttachment(<redacted>)")
    }
}

/// What an attachment record holds before the file's bytes.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordHead {
    item_id: Uuid,
    meta: AttachmentMeta,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordHeadRef<'a> {
    item_id: Uuid,
    meta: &'a AttachmentMeta,
}

pub const MAGIC: &[u8; 8] = b"HKBACKUP";
/// Version 2 adds attachment records (spec 2026-10-13-encrypted-attachments
/// §4.8). Version 1 files still open.
pub const FILE_VERSION: u8 = 2;
const KDF_ARGON2ID: u8 = 1;
/// magic(8) version(1) kdf(1) memory(4) iterations(4) parallelism(4) salt(16)
pub const HEADER_LEN: usize = 8 + 1 + 1 + 4 + 4 + 4 + SALT_LEN;
/// Version 2 adds record_count(4) to the header, so it is authenticated.
const V2_HEADER_LEN: usize = HEADER_LEN + 4;
/// The largest version 1 file.
pub const MAX_BACKUP_BYTES: u64 = crate::import::MAX_TEXT_EXPORT_BYTES;
/// The largest version 2 file: the items, every attachment, and framing.
pub const MAX_BACKUP_FILE_BYTES: u64 = MAX_BACKUP_BYTES + MAX_ATTACHMENT_VAULT_BYTES + 1024 * 1024;
const MAX_PAYLOAD_LEN: usize = blob::MAX_BACKUP_BLOB_LEN - blob::MIN_BLOB_LEN;

const NOT_A_BACKUP: Error = Error::InvalidInput("not a HavenKeys backup file");
const NEWER: Error = Error::InvalidInput("this backup was made by a newer version of HavenKeys");
const WRONG_PASSWORD: Error = Error::InvalidInput("wrong backup password, or the file is damaged");
/// The file decrypted, so the password was right and the bytes are what was
/// sealed; this version just cannot read what is inside.
const UNREADABLE: Error =
    Error::InvalidInput("this backup can't be read by this version of HavenKeys");
const TOO_LARGE: Error = Error::InvalidInput("backup file is too large");
const OUT_OF_ORDER: Error = Error::InvalidInput("backup records out of order");

fn header(kdf: &KdfParams, record_count: u32) -> [u8; V2_HEADER_LEN] {
    let mut h = [0u8; V2_HEADER_LEN];
    h[..8].copy_from_slice(MAGIC);
    h[8] = FILE_VERSION;
    h[9] = KDF_ARGON2ID;
    h[10..14].copy_from_slice(&kdf.memory_kib.to_le_bytes());
    h[14..18].copy_from_slice(&kdf.iterations.to_le_bytes());
    h[18..22].copy_from_slice(&kdf.parallelism.to_le_bytes());
    h[22..HEADER_LEN].copy_from_slice(&kdf.salt);
    h[HEADER_LEN..].copy_from_slice(&record_count.to_le_bytes());
    h
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// The file version and KDF parameters, checked in the order of spec §5.1.
fn parse_header(h: &[u8; HEADER_LEN]) -> Result<(u8, KdfParams)> {
    if &h[..8] != MAGIC {
        return Err(NOT_A_BACKUP);
    }
    match h[8] {
        1 | FILE_VERSION => {}
        v if v > FILE_VERSION => return Err(NEWER),
        _ => return Err(NOT_A_BACKUP),
    }
    if h[9] != KDF_ARGON2ID {
        return Err(NOT_A_BACKUP);
    }
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&h[22..HEADER_LEN]);
    let kdf = KdfParams {
        algorithm: KdfAlgorithm::Argon2id,
        memory_kib: u32_at(h, 10),
        iterations: u32_at(h, 14),
        parallelism: u32_at(h, 18),
        salt,
    };
    kdf.validate().map_err(|_| NOT_A_BACKUP)?;
    Ok((h[8], kdf))
}

/// Writes a version 2 backup record by record: the items first, then each
/// attachment, each sealed as it is written. Every record is bound to the
/// header (which holds the record count) and to its index.
pub struct BackupWriter<W: Write> {
    out: W,
    key: Key256,
    header: [u8; V2_HEADER_LEN],
    index: u32,
    count: u32,
    written: u64,
}

impl<W: Write> BackupWriter<W> {
    /// Derive the backup key and write the header. Slow (Argon2id): call
    /// without holding the vault lock.
    pub fn new(
        mut out: W,
        password: &SecretString,
        kdf: &KdfParams,
        attachment_count: u32,
    ) -> Result<Self> {
        let count = attachment_count.checked_add(1).ok_or(TOO_LARGE)?;
        let header = header(kdf, count);
        let key = derive_master_key(password, kdf)?;
        out.write_all(&header).map_err(|_| Error::Storage)?;
        Ok(Self {
            out,
            key,
            header,
            index: 0,
            count,
            written: V2_HEADER_LEN as u64,
        })
    }

    /// Record 0: a payload from `render(.., Backup, ..)`.
    pub fn items(&mut self, payload: &[u8]) -> Result<()> {
        if self.index != 0 {
            return Err(OUT_OF_ORDER);
        }
        if payload.len() > MAX_PAYLOAD_LEN {
            return Err(TOO_LARGE);
        }
        self.record(payload)
    }

    /// One attachment record: `u32 json_len ‖ {itemId, meta} ‖ bytes`.
    pub fn attachment(&mut self, item_id: Uuid, meta: &AttachmentMeta, bytes: &[u8]) -> Result<()> {
        if self.index == 0 || self.index >= self.count {
            return Err(OUT_OF_ORDER);
        }
        if bytes.len() > MAX_ATTACHMENT_BYTES || bytes.len() as u64 != meta.size {
            return Err(Error::InvalidInput("attachment is not valid"));
        }
        let head = serde_json::to_vec(&RecordHeadRef { item_id, meta })
            .map(Zeroizing::new)
            .map_err(|_| Error::Encryption)?;
        let mut plain = Zeroizing::new(Vec::with_capacity(4 + head.len() + bytes.len()));
        plain.extend_from_slice(&(head.len() as u32).to_le_bytes());
        plain.extend_from_slice(&head);
        plain.extend_from_slice(bytes);
        self.record(&plain)
    }

    /// Check every announced record was written and hand back the output.
    pub fn finish(mut self) -> Result<W> {
        if self.index != self.count {
            return Err(OUT_OF_ORDER);
        }
        self.out.flush().map_err(|_| Error::Storage)?;
        Ok(self.out)
    }

    fn record(&mut self, plain: &[u8]) -> Result<()> {
        let ctx = BlobContext::backup_record(&self.header, self.index);
        let sealed = blob::seal(&self.key, &ctx, plain)?;
        let written = self.written + 4 + sealed.len() as u64;
        if written > MAX_BACKUP_FILE_BYTES {
            return Err(TOO_LARGE);
        }
        self.out
            .write_all(&(sealed.len() as u32).to_le_bytes())
            .and_then(|()| self.out.write_all(&sealed))
            .map_err(|_| Error::Storage)?;
        self.written = written;
        self.index += 1;
        Ok(())
    }
}

/// A backup without attachments, in memory. Slow (Argon2id).
pub fn seal_backup(payload: &[u8], password: &SecretString, kdf: &KdfParams) -> Result<Vec<u8>> {
    // Refuse before the slow KDF.
    if payload.len() > MAX_PAYLOAD_LEN {
        return Err(TOO_LARGE);
    }
    let mut writer = BackupWriter::new(Vec::new(), password, kdf, 0)?;
    writer.items(payload)?;
    writer.finish()
}

/// The attachment records after a backup's items, read and opened one at a
/// time. A record that fails ends the iteration with its error; nothing from
/// such a file should be kept, since only reading to the end shows that no
/// record was dropped, moved or added.
pub struct BackupAttachments<R> {
    reader: R,
    key: Key256,
    header: Vec<u8>,
    index: u32,
    count: u32,
    read: u64,
    done: bool,
}

impl<R: Read> BackupAttachments<R> {
    /// A version 1 file's one blob: everything after the header.
    fn v1_items(&mut self) -> Result<Zeroizing<Vec<u8>>> {
        let cap = MAX_BACKUP_BYTES - HEADER_LEN as u64;
        let mut sealed = Vec::new();
        (&mut self.reader)
            .take(cap + 1)
            .read_to_end(&mut sealed)
            .map_err(|_| WRONG_PASSWORD)?;
        if sealed.len() as u64 > cap {
            return Err(TOO_LARGE);
        }
        self.index = 1;
        blob::open(&self.key, &BlobContext::backup(&self.header), &sealed)
            .map_err(|_| WRONG_PASSWORD)
    }

    /// The next version 2 record, opened; at most `max` bytes sealed.
    fn record(&mut self, max: usize) -> Result<Zeroizing<Vec<u8>>> {
        let mut len = [0u8; 4];
        self.reader
            .read_exact(&mut len)
            .map_err(|_| WRONG_PASSWORD)?;
        let len = u32::from_le_bytes(len) as usize;
        if !(blob::MIN_BLOB_LEN..=max).contains(&len) {
            return Err(WRONG_PASSWORD);
        }
        self.read += 4 + len as u64;
        if self.read > MAX_BACKUP_FILE_BYTES {
            return Err(TOO_LARGE);
        }
        let mut sealed = vec![0u8; len];
        self.reader
            .read_exact(&mut sealed)
            .map_err(|_| WRONG_PASSWORD)?;
        let ctx = BlobContext::backup_record(&self.header, self.index);
        self.index += 1;
        blob::open(&self.key, &ctx, &sealed).map_err(|_| WRONG_PASSWORD)
    }

    fn attachment(&mut self) -> Result<BackupAttachment> {
        let plain = self.record(blob::MAX_BLOB_LEN)?;
        parse_attachment(&plain).ok_or(UNREADABLE)
    }

    /// Nothing may follow the last record.
    fn end(&mut self) -> Result<()> {
        match self.reader.read(&mut [0u8; 1]) {
            Ok(0) => Ok(()),
            _ => Err(WRONG_PASSWORD),
        }
    }
}

impl<R: Read> Iterator for BackupAttachments<R> {
    type Item = Result<BackupAttachment>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        if self.index == self.count {
            self.done = true;
            return self.end().err().map(Err);
        }
        let attachment = self.attachment();
        self.done = attachment.is_err();
        Some(attachment)
    }
}

/// An authenticated attachment record's plaintext.
fn parse_attachment(plain: &[u8]) -> Option<BackupAttachment> {
    let (len, rest) = plain.split_first_chunk::<4>()?;
    let (head, bytes) = rest.split_at_checked(u32::from_le_bytes(*len) as usize)?;
    let RecordHead { item_id, meta } = serde_json::from_slice(head).ok()?;
    (bytes.len() <= MAX_ATTACHMENT_BYTES && bytes.len() as u64 == meta.size).then(|| {
        BackupAttachment {
            item_id,
            meta,
            bytes: SecretBytes::new(bytes.to_vec()),
        }
    })
}

/// Read a backup: hostile input, checked in the order of spec §5.1. Returns
/// the items and the attachment records still to be read (none in a version
/// 1 file). Slow (Argon2id).
pub fn open_backup_stream<R: Read>(
    mut reader: R,
    password: &SecretString,
) -> Result<(OpenedBackup, BackupAttachments<R>)> {
    let mut head = [0u8; HEADER_LEN];
    reader.read_exact(&mut head).map_err(|_| NOT_A_BACKUP)?;
    let (version, kdf) = parse_header(&head)?;
    let mut header = head.to_vec();
    let mut count = 1;
    if version == FILE_VERSION {
        let mut n = [0u8; 4];
        reader.read_exact(&mut n).map_err(|_| NOT_A_BACKUP)?;
        header.extend_from_slice(&n);
        count = u32::from_le_bytes(n);
        if count == 0 {
            return Err(NOT_A_BACKUP);
        }
    }
    let mut rest = BackupAttachments {
        reader,
        key: derive_master_key(password, &kdf)?,
        read: header.len() as u64,
        header,
        index: 0,
        count,
        done: false,
    };
    let plain = if version == FILE_VERSION {
        rest.record(blob::MAX_BACKUP_BLOB_LEN)?
    } else {
        rest.v1_items()?
    };
    Ok((parse_payload(&plain)?, rest))
}

/// The items record. From here on the AEAD has authenticated the bytes: a
/// failure is a format this version does not understand, never a wrong
/// password.
fn parse_payload(plain: &[u8]) -> Result<OpenedBackup> {
    // Version first, so a newer payload says so.
    #[derive(serde::Deserialize)]
    struct Version {
        version: u32,
    }
    let v: Version = serde_json::from_slice(plain).map_err(|_| UNREADABLE)?;
    if v.version != PAYLOAD_VERSION {
        return Err(if v.version > PAYLOAD_VERSION {
            NEWER
        } else {
            UNREADABLE
        });
    }
    let payload: Payload = serde_json::from_slice(plain).map_err(|_| UNREADABLE)?;
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
