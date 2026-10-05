//! Export commands (spec 2026-10-05-export). The renderer never supplies a
//! path: Rust opens the native save dialog and writes the file (0600 on
//! Unix, via a temporary file and a rename). Every export re-checks the
//! master password. Results are counts only.

use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::crypto::kdf::KdfParams;
use havenkeys_core::export::{self, backup, ExportFormat, ExportSummary};
use havenkeys_core::SecretString;
use serde::Serialize;
use std::io::Write;
use std::path::Path;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

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
/// the encrypted backup. Returns `None` if the save dialog was cancelled.
#[tauri::command]
pub async fn export_file(
    app: AppHandle,
    format: ExportFormat,
    master_password: SecretString,
    backup_password: Option<SecretString>,
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
    let rendered = export::render(
        &*app.state::<AppState>().vault()?,
        format,
        AppState::now_ms(),
    )?;
    let summary = rendered.summary;
    let write_path = path.clone();
    tauri::async_runtime::spawn_blocking(move || -> CmdResult<()> {
        match backup_password {
            Some(p) => {
                let sealed = backup::seal_backup(&rendered.bytes, &p, &KdfParams::generate()?)?;
                write_private(&write_path, &sealed)
            }
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

/// Write `bytes` to `path` readable by the owner only, replacing any file
/// there, without ever leaving a half-written file at `path`.
pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> CmdResult<()> {
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
    let result = (|| -> std::io::Result<()> {
        let mut f = options.open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(|_| CmdError::file())
}

#[cfg(test)]
mod tests {
    use super::write_private;

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
}
