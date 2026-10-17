//! Attachment metadata, file-name sanitizing and type sniffing
//! (spec 2026-10-13-encrypted-attachments).

use data_encoding::HEXLOWER;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;
use zeroize::Zeroize;

/// Largest plaintext file.
pub const MAX_ATTACHMENT_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_ATTACHMENTS_PER_ITEM: usize = 20;
/// Committed plus pending attachment bytes per vault.
pub const MAX_ATTACHMENT_VAULT_BYTES: u64 = 192 * 1024 * 1024;

const MAX_NAME_CHARS: usize = 120;
const MAX_EXTENSION_CHARS: usize = 10;
const FALLBACK_NAME: &str = "attachment";

/// What an item's details record about one attached file. The file itself is a
/// separate blob, identified by `id` and checked against `blob_sha256`.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttachmentMeta {
    pub id: Uuid,
    pub name: String,
    pub mime: String,
    pub size: u64,
    #[serde(with = "hex")]
    pub blob_sha256: [u8; 32],
    pub created_at: i64,
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

fn is_hidden(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
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
    let name = cleaned.trim_matches(|c: char| c.is_whitespace() || c == '.');
    if name.is_empty() {
        return FALLBACK_NAME.into();
    }
    cap_length(name)
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
