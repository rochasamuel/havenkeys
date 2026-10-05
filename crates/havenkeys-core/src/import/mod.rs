//! Importers for other password managers' export files.
//!
//! Exports are plaintext and treated as hostile input: size-limited, parsed
//! defensively, never logged. Parsed values go through the same validation as
//! items typed into the UI, and storing them needs the server exactly as a
//! single write does: `VaultService::stage_import` seals them, and the
//! desktop sends them in batches the server accepts (spec 2026-09-20 §8.4).

pub mod bitwarden;
mod common;
pub mod csv;
pub mod onepux;

use crate::error::{Error, Result};
use crate::model::ItemInput;
use serde::{Deserialize, Serialize};

/// Most items one export may hold.
pub const MAX_ITEMS: usize = 50_000;
/// Largest JSON or CSV export accepted.
pub const MAX_TEXT_EXPORT_BYTES: u64 = 64 * 1024 * 1024;

/// Where an export came from. The user picks it; the file must then be that
/// source's export or it is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportSource {
    /// `.1pux`
    OnePassword,
    /// Unencrypted `.json`
    BitwardenJson,
    BitwardenCsv,
    /// Chrome, Edge, Brave and other Chromium browsers.
    Chrome,
    Firefox,
    KeePassXc,
    LastPass,
}

impl ImportSource {
    /// Largest file accepted for this source.
    pub fn max_bytes(self) -> u64 {
        match self {
            ImportSource::OnePassword => onepux::MAX_ARCHIVE_BYTES,
            _ => MAX_TEXT_EXPORT_BYTES,
        }
    }

    /// The file extension its export uses.
    pub fn extension(self) -> &'static str {
        match self {
            ImportSource::OnePassword => "1pux",
            ImportSource::BitwardenJson => "json",
            _ => "csv",
        }
    }

    /// Product name, for the file picker.
    pub fn name(self) -> &'static str {
        match self {
            ImportSource::OnePassword => "1Password",
            ImportSource::BitwardenJson | ImportSource::BitwardenCsv => "Bitwarden",
            ImportSource::Chrome => "Chrome",
            ImportSource::Firefox => "Firefox",
            ImportSource::KeePassXc => "KeePassXC",
            ImportSource::LastPass => "LastPass",
        }
    }
}

/// Parse result: items to store plus what was skipped while parsing.
/// `report.imported`/`failed`/`skipped_duplicates` are filled in by the vault.
pub struct Parsed {
    pub items: Vec<ImportedItem>,
    pub report: ImportReport,
}

/// Parse `bytes` as `source`'s export.
pub fn parse(source: ImportSource, bytes: &[u8]) -> Result<Parsed> {
    if bytes.len() as u64 > source.max_bytes() {
        return Err(Error::InvalidInput("export file is too large"));
    }
    match source {
        ImportSource::OnePassword => onepux::parse(bytes),
        ImportSource::BitwardenJson => bitwarden::parse(bytes),
        ImportSource::BitwardenCsv
        | ImportSource::Chrome
        | ImportSource::Firefox
        | ImportSource::KeePassXc
        | ImportSource::LastPass => csv::parse(source, bytes),
    }
}

/// One item ready to be stored, with its original timestamps (Unix ms).
pub struct ImportedItem {
    pub input: ItemInput,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}

impl std::fmt::Debug for ImportedItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ImportedItem(<redacted>)")
    }
}

/// Counts only; never item content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    /// Items written to the vault.
    pub imported: usize,
    pub logins: usize,
    pub secure_notes: usize,
    /// Credit cards imported as Cards.
    pub cards: usize,
    /// Items of other kinds (identities, SSH keys, …) stored as secure notes
    /// with their fields written out.
    pub converted_to_notes: usize,
    /// Already present in the vault before the import (same title, username and
    /// websites for logins; same title and content for notes).
    pub skipped_duplicates: usize,
    /// Archived or deleted in the source.
    pub skipped_archived: usize,
    /// Items that could not be imported (e.g. a field over the size limits).
    pub failed: usize,
    /// File attachments are not supported and were left out.
    pub attachments_skipped: usize,
    /// Old passwords from password history were left out.
    pub password_history_skipped: usize,
    /// Passkeys in the export were left out (importing them is not
    /// supported yet).
    pub passkeys_skipped: usize,
    /// Website entries that were not valid http(s) addresses; kept as text in
    /// the item's notes instead of as matchable websites.
    pub urls_moved_to_notes: usize,
    /// Custom fields past a login's limits (100 fields, 20 sections), kept as
    /// text in the login's notes instead.
    pub fields_to_notes: usize,
    /// Logins already in the vault (same title, username and websites) that
    /// lacked "Sign in with" and got it from this import.
    pub sso_upgraded: usize,
}
