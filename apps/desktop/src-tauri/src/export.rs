//! Export commands (spec 2026-10-05-export). The renderer never supplies a
//! path: Rust opens the native save dialog and writes the file (0600 on
//! Unix, via a temporary file and a rename). Every export re-checks the
//! master password. Results are counts only.

use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_client::HavenClient;
use havenkeys_core::attachment::AttachmentMeta;
use havenkeys_core::crypto::kdf::KdfParams;
use havenkeys_core::export::backup::BackupWriter;
use havenkeys_core::export::{self, ExportFormat, ExportSummary};
use havenkeys_core::SecretString;
use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    /// File name only (no directory), for the confirmation message.
    file_name: String,
    summary: ExportSummary,
}

/// What an export in `format` would hold and leave out.
#[tauri::command]
pub fn export_summary(
    state: tauri::State<'_, AppState>,
    format: ExportFormat,
) -> CmdResult<ExportSummary> {
    state.touch();
    state.require_unlocked()?;
    Ok(export::summarize(&*state.vault()?, format)?)
}

/// Export the vault. `backup_password` is required for (and only used by)
/// the encrypted backup, which carries the attached files unless
/// `include_attachments` is false. Returns `None` if the save dialog was
/// cancelled.
#[tauri::command]
pub async fn export_file(
    app: AppHandle,
    format: ExportFormat,
    master_password: SecretString,
    backup_password: Option<SecretString>,
    include_attachments: bool,
) -> CmdResult<Option<ExportResult>> {
    {
        let state = app.state::<AppState>();
        state.touch();
        state.require_unlocked()?;
    }
    let backup_password = match (format, backup_password) {
        (ExportFormat::Backup, Some(p)) => {
            export::check_backup_password(&p, &master_password)?;
            Some(p)
        }
        (ExportFormat::Backup, None) => {
            return Err(
                havenkeys_core::Error::InvalidInput("a backup needs a backup password").into(),
            )
        }
        (_, _) => None,
    };
    app.state::<AppState>()
        .client()
        .verify_master_password(master_password)
        .await?;

    let handle = app.clone();
    let file_name = export::default_file_name(format, AppState::now_ms());
    let picked = tauri::async_runtime::spawn_blocking(move || {
        handle
            .dialog()
            .file()
            .set_title(format!("Export as {}", format.name()))
            .set_file_name(file_name)
            .add_filter(format.name(), &[format.extension()])
            .blocking_save_file()
    })
    .await
    .map_err(|_| CmdError::internal())?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|_| CmdError::file())?;

    // Rendered under the vault lock (fast); a lock since the password check
    // answers `Locked` here and nothing is written.
    let (rendered, files) = {
        let state = app.state::<AppState>();
        let vault = state.vault()?;
        let rendered = export::render(&vault, format, AppState::now_ms())?;
        let files = match backup_password {
            Some(_) if include_attachments => export::backup_attachments(&vault)?,
            _ => Vec::new(),
        };
        (rendered, files)
    };
    let mut summary = rendered.summary;
    if format == ExportFormat::Backup && !include_attachments {
        summary.attachments_left_out = std::mem::take(&mut summary.attachments);
    }
    let client = app.state::<AppState>().client().clone();
    let write_path = path.clone();
    tauri::async_runtime::spawn_blocking(move || -> CmdResult<()> {
        match backup_password {
            Some(p) => write_private_with(&write_path, |file| {
                write_backup(file, &rendered.bytes, &p, &client, &files)
            }),
            None => write_private(&write_path, &rendered.bytes),
        }
    })
    .await
    .map_err(|_| CmdError::internal())??;

    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Some(ExportResult { file_name, summary }))
}

/// Seal the backup record by record into `file`: the items, then each
/// attached file, fetched (cache or server) as its turn comes so only one is
/// in memory at a time. Runs on a blocking thread; any file that cannot be
/// read fails the export, and the caller leaves no file behind.
fn write_backup(
    file: &mut File,
    payload: &[u8],
    password: &SecretString,
    client: &Arc<HavenClient>,
    files: &[(Uuid, AttachmentMeta)],
) -> CmdResult<()> {
    let count = u32::try_from(files.len()).map_err(|_| CmdError::internal())?;
    let mut writer = BackupWriter::new(file, password, &KdfParams::generate()?, count)?;
    writer.items(payload)?;
    for (item_id, meta) in files {
        let bytes = tauri::async_runtime::block_on(client.attachment_bytes(*item_id, meta.id))
            .map_err(|_| CmdError::attachments_unavailable())?;
        writer.attachment(*item_id, meta, bytes.expose())?;
    }
    writer.finish()?;
    Ok(())
}

/// Write `bytes` to `path` readable by the owner only, replacing any file
/// there, without ever leaving a half-written file at `path`.
pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> CmdResult<()> {
    write_private_with(path, |file| {
        file.write_all(bytes).map_err(|_| CmdError::file())
    })
}

/// Like `write_private`, with the contents written by `fill`. If `fill`
/// fails nothing is left at `path`.
fn write_private_with(path: &Path, fill: impl FnOnce(&mut File) -> CmdResult<()>) -> CmdResult<()> {
    let dir = path.parent().ok_or_else(CmdError::file)?;
    let name = path
        .file_name()
        .ok_or_else(CmdError::file)?
        .to_string_lossy();
    let tmp = dir.join(format!(".{name}.{}.part", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> CmdResult<()> {
        let mut f = options.open(&tmp).map_err(|_| CmdError::file())?;
        fill(&mut f)?;
        f.sync_all().map_err(|_| CmdError::file())?;
        std::fs::rename(&tmp, path).map_err(|_| CmdError::file())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{write_private, write_private_with};
    use crate::state::CmdError;

    #[test]
    fn writes_replace_and_are_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.csv");
        std::fs::write(&path, b"old").unwrap();
        write_private(&path, b"new contents").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new contents");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        // No temporary file left behind.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn a_failed_fill_leaves_nothing_and_keeps_the_old_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.hkbackup");
        std::fs::write(&path, b"old").unwrap();
        let failed = write_private_with(&path, |_| Err(CmdError::attachments_unavailable()));
        assert_eq!(failed.unwrap_err().code, "attachments_unavailable");
        assert_eq!(std::fs::read(&path).unwrap(), b"old");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
