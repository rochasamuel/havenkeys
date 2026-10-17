//! Attachment metadata, file-name sanitizing and type sniffing
//! (spec 2026-10-13-encrypted-attachments).

use crate::crypto::blob::{self, BlobContext};
use crate::error::{Error, Result};
use crate::model::ItemOverview;
use crate::secret::SecretBytes;
use crate::vault::{Session, StagedWrite, VaultService};
use data_encoding::HEXLOWER;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;
use uuid::Uuid;
use zeroize::Zeroize;

/// Largest plaintext file.
pub const MAX_ATTACHMENT_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_ATTACHMENTS_PER_ITEM: usize = 20;
/// Committed plus pending attachment bytes per vault.
pub const MAX_ATTACHMENT_VAULT_BYTES: u64 = 192 * 1024 * 1024;

/// Sealed attachments a desktop keeps for offline opening.
pub const DESKTOP_CACHE_BYTES: u64 = 64 * 1024 * 1024;

const MAX_NAME_CHARS: usize = 120;
const MAX_EXTENSION_CHARS: usize = 10;
const FALLBACK_NAME: &str = "attachment";

/// What an item's details record about one attached file. The file itself is a
/// separate blob, identified by `id` and checked against `blob_sha256`.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentMeta {
    pub id: Uuid,
    pub name: String,
    pub mime: String,
    pub size: u64,
    #[serde(with = "hex")]
    pub blob_sha256: [u8; 32],
    pub created_at: i64,
    /// Fields written by a newer version that this one does not know. Kept
    /// so an edit made here does not erase them.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl fmt::Debug for AttachmentMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttachmentMeta")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Drop for AttachmentMeta {
    fn drop(&mut self) {
        self.name.zeroize();
    }
}

mod hex {
    use super::HEXLOWER;
    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(hash: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&HEXLOWER.encode(hash))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let text = String::deserialize(d)?;
        HEXLOWER
            .decode(text.as_bytes())
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| D::Error::custom("invalid hash"))
    }
}

const TRIMMED: fn(char) -> bool = |c| c.is_whitespace() || c == '.';

fn is_hidden(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{61C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        )
}

/// Windows refuses these as file names, with or without an extension.
fn is_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or_default().trim_end();
    let numbered = |prefix: &str| {
        stem.len() == 4
            && stem.is_ascii()
            && stem[..3].eq_ignore_ascii_case(prefix)
            && matches!(stem.as_bytes()[3], b'1'..=b'9')
    };
    ["CON", "PRN", "AUX", "NUL"]
        .iter()
        .any(|d| stem.eq_ignore_ascii_case(d))
        || numbered("COM")
        || numbered("LPT")
}

/// A display and save name that is safe to show and to write to disk: the last
/// path component, without control or direction-override characters, reserved
/// characters replaced, at most 120 characters with a short extension kept.
pub fn sanitize_name(raw: &str) -> String {
    let last = raw.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = last
        .chars()
        .filter(|&c| !is_hidden(c))
        .map(|c| if "<>:\"|?*".contains(c) { '_' } else { c })
        .collect();
    let name = cleaned.trim_matches(TRIMMED);
    if name.is_empty() {
        return FALLBACK_NAME.into();
    }
    let capped = cap_length(name);
    let name = capped.trim_end_matches(TRIMMED);
    if is_device_name(name) {
        format!("_{name}")
    } else {
        name.into()
    }
}

fn cap_length(name: &str) -> String {
    if name.chars().count() <= MAX_NAME_CHARS {
        return name.into();
    }
    let ext = name
        .rfind('.')
        .map(|i| &name[i..])
        .filter(|e| e.chars().count() <= MAX_EXTENSION_CHARS)
        .unwrap_or_default();
    let stem_len = MAX_NAME_CHARS - ext.chars().count();
    name.chars().take(stem_len).chain(ext.chars()).collect()
}

/// The file's type from its leading bytes. The name and any type the user
/// supplied are never trusted.
pub fn sniff(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(b"\xFF\xD8\xFF") {
        "image/jpeg"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.starts_with(b"%PDF-") {
        "application/pdf"
    } else {
        "application/octet-stream"
    }
}

/// Only raster images are rendered inline; SVG and PDF never are.
pub fn is_previewable(mime: &str) -> bool {
    matches!(
        mime,
        "image/png" | "image/jpeg" | "image/webp" | "image/gif"
    )
}

/// A file sealed for upload: what the item's details record, and the blob the
/// server stores.
pub struct SealedAttachment {
    pub meta: AttachmentMeta,
    pub blob: Vec<u8>,
}

/// An edit to an item's list of files.
pub enum AttachmentChange {
    Add(Vec<AttachmentMeta>),
    Remove(Uuid),
}

impl VaultService {
    /// Seal one file for `item_id`. The name and type are not trusted: the
    /// name is sanitized and the type comes from the bytes.
    pub fn seal_attachment(
        &self,
        item_id: &Uuid,
        name: &str,
        bytes: &[u8],
        now_ms: i64,
    ) -> Result<SealedAttachment> {
        if bytes.len() > MAX_ATTACHMENT_BYTES {
            return Err(Error::InvalidInput("Files can be up to 5 MB"));
        }
        let session = self.session()?;
        let id = Uuid::new_v4();
        let ctx = BlobContext::attachment(session.vault_id, *item_id, id);
        let blob = blob::seal(&session.data_key, &ctx, bytes)?;
        let meta = AttachmentMeta {
            id,
            name: sanitize_name(name),
            mime: sniff(bytes).into(),
            size: bytes.len() as u64,
            blob_sha256: Sha256::digest(&blob).into(),
            created_at: now_ms,
            extra: serde_json::Map::new(),
        };
        Ok(SealedAttachment { meta, blob })
    }

    /// Open a downloaded blob. A blob the server altered or swapped fails the
    /// hash check; one that passes it still has to authenticate for this item.
    pub fn open_attachment(
        &self,
        item_id: &Uuid,
        meta: &AttachmentMeta,
        blob: &[u8],
    ) -> Result<SecretBytes> {
        if Sha256::digest(blob)[..] != meta.blob_sha256 {
            return Err(Error::Decryption);
        }
        let session = self.session()?;
        let ctx = BlobContext::attachment(session.vault_id, *item_id, meta.id);
        let plain = blob::open(&session.data_key, &ctx, blob)?;
        Ok(SecretBytes::new(plain.to_vec()))
    }

    /// One attached file's record, from a live or trashed item.
    pub fn attachment_meta(&self, item_id: &Uuid, attachment_id: &Uuid) -> Result<AttachmentMeta> {
        stored_overview(self.session()?, item_id)?
            .attachments
            .iter()
            .find(|meta| meta.id == *attachment_id)
            .cloned()
            .ok_or(Error::NotFound)
    }

    /// The sealed copy this device downloaded earlier, if it still has one.
    pub fn cached_attachment(&self, attachment_id: &Uuid) -> Result<Option<Vec<u8>>> {
        self.session()?;
        self.store.cached_attachment(attachment_id)
    }

    pub fn cache_attachment(&self, attachment_id: &Uuid, blob: &[u8], now_ms: i64) -> Result<()> {
        self.session()?;
        self.store
            .put_cached_attachment(attachment_id, blob, now_ms)
    }

    pub fn evict_attachment(&self, attachment_id: &Uuid) -> Result<()> {
        self.session()?;
        self.store.delete_cached_attachment(attachment_id)
    }

    /// Forget copies of files no live or trashed item holds, then the least
    /// recently used until the cache fits `cap_bytes`.
    pub fn prune_attachment_cache(&self, cap_bytes: u64) -> Result<()> {
        let session = self.session()?;
        let keep: HashSet<Uuid> = session
            .overviews
            .values()
            .chain(session.trash.values())
            .flat_map(|ov| ov.attachments.iter().map(|meta| meta.id))
            .collect();
        self.store.prune_attachment_cache(&keep, cap_bytes)
    }

    /// Stage the item with its list of files changed and everything else as
    /// it was. Carries every id the new list holds.
    pub fn stage_attachments(
        &self,
        item_id: &Uuid,
        change: AttachmentChange,
        now_ms: i64,
    ) -> Result<StagedWrite> {
        let session = self.session()?;
        let mut overview = stored_overview(session, item_id)?.clone();
        match change {
            AttachmentChange::Add(metas) => {
                if overview.attachments.len() + metas.len() > MAX_ATTACHMENTS_PER_ITEM {
                    return Err(Error::InvalidInput("An item holds up to 20 files"));
                }
                overview.attachments.extend(metas);
            }
            AttachmentChange::Remove(id) => {
                let before = overview.attachments.len();
                overview.attachments.retain(|meta| meta.id != id);
                if overview.attachments.len() == before {
                    return Err(Error::NotFound);
                }
            }
        }
        overview.updated_at = now_ms;
        let ids = overview.attachments.iter().map(|m| m.id).collect();
        let base = self.item_revision(item_id)?;
        let details = if overview.trashed_at.is_some() {
            self.load_trashed_details(item_id)?
        } else {
            self.load_details(item_id)?
        };
        let mut staged = self.seal_staged(overview, Some(&details), base)?;
        staged.attachment_ids = Some(ids);
        Ok(staged)
    }
}

fn stored_overview<'a>(session: &'a Session, id: &Uuid) -> Result<&'a ItemOverview> {
    session
        .overviews
        .get(id)
        .or_else(|| session.trash.get(id))
        .ok_or(Error::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_keeps_last_component() {
        assert_eq!(sanitize_name("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_name("C:\\x\\a.pdf"), "a.pdf");
    }

    #[test]
    fn sanitize_strips_controls_and_bidi() {
        assert_eq!(sanitize_name("in\u{202E}fdp.exe"), "infdp.exe");
        assert_eq!(sanitize_name("a\nb.txt"), "ab.txt");
    }

    #[test]
    fn sanitize_replaces_reserved() {
        assert_eq!(sanitize_name("a:b*c?.png"), "a_b_c_.png");
    }

    #[test]
    fn sanitize_trims_and_falls_back() {
        assert_eq!(sanitize_name(" . "), "attachment");
        assert_eq!(sanitize_name(""), "attachment");
    }

    #[test]
    fn sanitize_strips_invisible_characters() {
        let name = "a\u{61C}\u{200B}\u{200C}\u{200D}\u{2028}\u{2029}\u{2060}\u{FEFF}b.txt";
        assert_eq!(sanitize_name(name), "ab.txt");
    }

    #[test]
    fn sanitize_retrims_after_truncation() {
        let name = sanitize_name(&format!("{} {}", "x".repeat(119), "y".repeat(9)));
        assert_eq!(name, "x".repeat(119));
    }

    #[test]
    fn sanitize_prefixes_windows_device_names() {
        for raw in ["CON", "nul.txt", "Com1.tar", "LPT9", "aux.", "PRN .png"] {
            assert!(sanitize_name(raw).starts_with('_'), "{raw}");
        }
        assert_eq!(sanitize_name("CON.txt"), "_CON.txt");
        assert_eq!(sanitize_name("console.txt"), "console.txt");
        assert_eq!(sanitize_name("COM0"), "COM0");
    }

    #[test]
    fn sanitize_handles_multibyte_short_stems() {
        for raw in [
            "abé.pdf",
            "çon.txt",
            "éé.png",
            "a\u{1F512}.txt",
            "ab\u{1F512}",
        ] {
            assert_eq!(sanitize_name(raw), raw);
        }
    }

    #[test]
    fn sanitize_caps_length_keeping_extension() {
        let name = sanitize_name(&format!("{}.jpeg", "x".repeat(295)));
        assert_eq!(name.chars().count(), 120);
        assert!(name.ends_with(".jpeg"));
    }

    #[test]
    fn sniff_by_magic() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\nrest"), "image/png");
        assert_eq!(sniff(b"\xFF\xD8\xFF\xE0"), "image/jpeg");
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), "image/webp");
        assert_eq!(sniff(b"GIF89a.."), "image/gif");
        assert_eq!(sniff(b"GIF87a.."), "image/gif");
        assert_eq!(sniff(b"%PDF-1.7"), "application/pdf");
        assert_eq!(sniff(b"<svg"), "application/octet-stream");
    }

    #[test]
    fn only_images_preview() {
        assert!(is_previewable("image/png"));
        assert!(!is_previewable("application/pdf"));
        assert!(!is_previewable("image/svg+xml"));
    }
}
