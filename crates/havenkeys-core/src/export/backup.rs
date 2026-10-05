//! The encrypted HavenKeys backup (`.hkbackup`), spec 2026-10-05-export §5.

use crate::model::{ItemDetails, ItemOverview};
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

#[allow(dead_code)] // read by restore (Task 4)
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Payload {
    pub version: u32,
    pub exported_at: i64,
    pub items: Vec<BackupItem>,
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
