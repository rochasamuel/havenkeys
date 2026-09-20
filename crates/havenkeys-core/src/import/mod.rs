//! Importers for other password managers' export files.
//!
//! Exports are plaintext and treated as hostile input: size-limited, parsed
//! defensively, never logged. Parsed values go through the same validation as
//! items typed into the UI. Storing them needs the server exactly as a
//! single write does (spec 2026-09-20 §8.4); until the sync client exists
//! (§13), `import_1pux` refuses with `Error::Offline`.

pub mod onepux;

use crate::model::ItemInput;
use serde::Serialize;

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
    /// Items of other kinds (credit cards, identities, SSH keys, …) stored as
    /// secure notes with their fields written out.
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
    /// Website entries that were not valid http(s) addresses; kept as text in
    /// the item's notes instead of as matchable websites.
    pub urls_moved_to_notes: usize,
}
