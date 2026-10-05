//! Exporting the vault (spec 2026-10-05-export): an encrypted HavenKeys
//! backup, or plaintext Bitwarden JSON / CSV for other password managers.
//!
//! Plaintext exports never contain passkey private keys. Nothing here logs;
//! errors are fixed strings; summaries are counts only.

pub mod backup;
mod bitwarden;
mod restore;

use crate::error::{Error, Result};
use crate::import::common::format_date;
use crate::model::{ItemDetails, ItemOverview, ItemType};
use crate::secret::SecretString;
use crate::vault::{
    secrets_equal, VaultService, MAX_MASTER_PASSWORD_CHARS, MIN_MASTER_PASSWORD_CHARS,
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// The backup password: as long as a master password may be, and not the
/// master password itself (a backup must not fall with the account).
pub fn check_backup_password(backup: &SecretString, master: &SecretString) -> Result<()> {
    let n = backup.char_len();
    if n < MIN_MASTER_PASSWORD_CHARS {
        return Err(Error::InvalidInput(
            "backup password must be at least 10 characters",
        ));
    }
    if n > MAX_MASTER_PASSWORD_CHARS {
        return Err(Error::InvalidInput("backup password is too long"));
    }
    if secrets_equal(backup, master) {
        return Err(Error::InvalidInput(
            "backup password must differ from the master password",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    /// Encrypted, for HavenKeys; carries everything, passkeys included.
    Backup,
    /// Unencrypted Bitwarden JSON, for other password managers.
    BitwardenJson,
    /// Logins only (`name,url,username,password,note,totp`).
    Csv,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Backup => "hkbackup",
            ExportFormat::BitwardenJson => "json",
            ExportFormat::Csv => "csv",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ExportFormat::Backup => "HavenKeys backup",
            ExportFormat::BitwardenJson => "Bitwarden JSON",
            ExportFormat::Csv => "CSV",
        }
    }
}

/// What an export holds and leaves out. Counts only, never content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    pub logins: usize,
    pub secure_notes: usize,
    pub cards: usize,
    pub identities: usize,
    /// Never in a plaintext export.
    pub passkeys_left_out: usize,
    pub password_history_left_out: usize,
    pub custom_fields_left_out: usize,
    /// Notes, cards and the identity, for CSV.
    pub items_left_out: usize,
    /// Items whose encrypted details no longer open; not exported.
    pub unreadable: usize,
}

pub struct Rendered {
    pub bytes: Zeroizing<Vec<u8>>,
    pub summary: ExportSummary,
}

pub fn default_file_name(format: ExportFormat, now_ms: i64) -> String {
    format!(
        "havenkeys-export-{}.{}",
        format_date(now_ms.div_euclid(1000)),
        format.extension()
    )
}

/// Visit every item with its details (`None` if they no longer open), in a
/// stable order.
pub(crate) fn for_each_item(
    vault: &VaultService,
    mut f: impl FnMut(&ItemOverview, Option<ItemDetails>) -> Result<()>,
) -> Result<()> {
    let session = vault.session()?;
    let mut overviews: Vec<&ItemOverview> = session.overviews.values().collect();
    overviews.sort_by_key(|o| (o.created_at, o.id));
    for ov in overviews {
        f(ov, vault.load_details(&ov.id).ok())?;
    }
    Ok(())
}

/// Count one item into `s` for `format`.
fn count(s: &mut ExportSummary, format: ExportFormat, ov: &ItemOverview, d: &ItemDetails) {
    match ov.item_type {
        ItemType::Login => s.logins += 1,
        ItemType::SecureNote => s.secure_notes += 1,
        ItemType::Card => s.cards += 1,
        ItemType::Identity => s.identities += 1,
    }
    if format == ExportFormat::Backup {
        return;
    }
    if let ItemDetails::Login {
        passkeys,
        password_history,
        sections,
        ..
    } = d
    {
        s.passkeys_left_out += passkeys.len();
        if format == ExportFormat::Csv {
            s.password_history_left_out += password_history.len();
            s.custom_fields_left_out += sections.iter().map(|x| x.fields.len()).sum::<usize>();
        }
    } else if format == ExportFormat::Csv {
        s.items_left_out += 1;
    }
}

pub fn summarize(vault: &VaultService, format: ExportFormat) -> Result<ExportSummary> {
    let mut s = ExportSummary::default();
    for_each_item(vault, |ov, d| {
        match d {
            Some(d) => count(&mut s, format, ov, &d),
            None => s.unreadable += 1,
        }
        Ok(())
    })?;
    Ok(s)
}

pub fn render(vault: &VaultService, format: ExportFormat, now_ms: i64) -> Result<Rendered> {
    vault.session()?;
    match format {
        ExportFormat::Backup => render_backup_payload(vault, now_ms),
        ExportFormat::BitwardenJson => bitwarden::render(vault),
        ExportFormat::Csv => csv::render(vault),
    }
}

fn render_backup_payload(vault: &VaultService, now_ms: i64) -> Result<Rendered> {
    let mut summary = ExportSummary::default();
    let mut kept: Vec<(ItemOverview, ItemDetails)> = Vec::new();
    for_each_item(vault, |ov, d| {
        match d {
            Some(d) => {
                count(&mut summary, ExportFormat::Backup, ov, &d);
                kept.push((ov.clone(), d));
            }
            None => summary.unreadable += 1,
        }
        Ok(())
    })?;
    let refs: Vec<backup::BackupItemRef<'_>> = kept
        .iter()
        .map(|(overview, details)| backup::BackupItemRef { overview, details })
        .collect();
    let mut bytes = Zeroizing::new(Vec::new());
    serde_json::to_writer(
        &mut *bytes,
        &backup::PayloadRef {
            version: backup::PAYLOAD_VERSION,
            exported_at: now_ms,
            items: &refs,
        },
    )
    .map_err(|_| Error::Encryption)?;
    Ok(Rendered { bytes, summary })
}

// Temporary stub, replaced in Task 7.
mod csv {
    pub(super) fn render(_: &crate::vault::VaultService) -> crate::error::Result<super::Rendered> {
        Err(crate::error::Error::InvalidInput("format not available"))
    }
}
