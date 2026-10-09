//! Import commands. The renderer never supplies a file path: Rust opens the
//! native file picker, reads the chosen file into memory, and remembers the
//! path only so the user can choose to delete the plaintext export afterwards.
//!
//! Imported items are written exactly like typed ones — sealed under the
//! vault lock, sent to the server, recorded once it has accepted them — in
//! batches of at most 500 (spec 2026-09-20 §8.4).

use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::import::{self, ImportReport, ImportSource};
use havenkeys_core::SecretString;
use serde::Serialize;
use std::io::Read;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use zeroize::Zeroizing;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    report: ImportReport,
    /// File name only (no directory), for the confirmation message.
    file_name: String,
}

fn read_limited(path: &std::path::Path, limit: u64) -> CmdResult<Zeroizing<Vec<u8>>> {
    let meta = std::fs::metadata(path).map_err(|_| CmdError::file())?;
    if !meta.is_file() {
        return Err(CmdError::file());
    }
    if meta.len() > limit {
        return Err(havenkeys_core::Error::InvalidInput("export file is too large").into());
    }
    let file = std::fs::File::open(path).map_err(|_| CmdError::file())?;
    let mut buf = Zeroizing::new(Vec::with_capacity(meta.len() as usize));
    file.take(limit + 1)
        .read_to_end(&mut buf)
        .map_err(|_| CmdError::file())?;
    Ok(buf)
}

/// Let the user pick an export from `source` and import it. `source` is a
/// closed set; the file must be that source's export or nothing is stored.
/// Returns `None` if the picker was cancelled.
#[tauri::command]
pub async fn import_file(app: AppHandle, source: ImportSource) -> CmdResult<Option<ImportResult>> {
    {
        let state = app.state::<AppState>();
        state.touch();
        state.require_unlocked()?;
        // Checked before the picker opens: asking for a file and then
        // refusing to store it would waste the user's time and leave a
        // plaintext export sitting on disk for nothing.
        state.require_online()?;
        state.require_full()?;
    }

    let handle = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        handle
            .dialog()
            .file()
            .set_title(format!("Import from {}", source.name()))
            .add_filter(format!("{} export", source.name()), &[source.extension()])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| CmdError::internal())?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|_| CmdError::file())?;

    // Read and parse off the async runtime; the vault lock is only taken to seal.
    let parse_path = path.clone();
    let parsed = tauri::async_runtime::spawn_blocking(move || -> CmdResult<import::Parsed> {
        let bytes = read_limited(&parse_path, source.max_bytes())?;
        Ok(import::parse(source, &bytes)?)
    })
    .await
    .map_err(|_| CmdError::internal())??;

    let staged = app.state::<AppState>().vault()?.stage_import(
        parsed.items,
        parsed.report,
        AppState::now_ms(),
    )?;
    let mut report = staged.report;
    let committed = app
        .state::<AppState>()
        .client()
        .clone()
        .push_batches(staged.writes)
        .await?;
    // What the server accepted is what the vault has; the staged count was a
    // forecast. Upgrades (already-present logins that gained sign_in_with)
    // are counted separately, not as newly imported items.
    report.imported = committed.saturating_sub(report.sso_upgraded);

    let state = app.state::<AppState>();
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Ok(mut last) = state.last_import.lock() {
        *last = Some(path);
    }
    Ok(Some(ImportResult { report, file_name }))
}

/// Delete the export file chosen in the last `import_file` call. This is a
/// normal file deletion, not a secure wipe (see docs/security-model.md).
#[tauri::command]
pub fn delete_import_file(state: tauri::State<'_, AppState>) -> CmdResult<()> {
    state.touch();
    // Documented as requiring an unlocked vault (`security-model.md` §7), and
    // enforced here rather than left to the renderer. `lock()` also clears the
    // remembered path, so this fails with `NotFound` after a lock either way.
    state.require_unlocked()?;
    let path = state
        .last_import
        .lock()
        .map_err(|_| CmdError::internal())?
        .take()
        .ok_or(havenkeys_core::Error::NotFound)?;
    std::fs::remove_file(&path).map_err(|_| CmdError::file())
}

/// Restore an encrypted HavenKeys backup. Like `import_file`, Rust picks the
/// file; items already in the vault are skipped, nothing is overwritten.
/// The backup is meant to be kept, so no "delete the file" offer follows.
#[tauri::command]
pub async fn restore_backup(
    app: AppHandle,
    backup_password: SecretString,
) -> CmdResult<Option<ImportResult>> {
    {
        let state = app.state::<AppState>();
        state.touch();
        state.require_unlocked()?;
        state.require_online()?;
        state.require_full()?;
    }
    let handle = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        handle
            .dialog()
            .file()
            .set_title("Restore a HavenKeys backup")
            .add_filter("HavenKeys backup", &["hkbackup"])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| CmdError::internal())?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|_| CmdError::file())?;
    let read_path = path.clone();
    let opened = tauri::async_runtime::spawn_blocking(move || -> CmdResult<_> {
        let bytes = read_limited(&read_path, havenkeys_core::export::backup::MAX_BACKUP_BYTES)?;
        Ok(havenkeys_core::export::backup::open_backup(
            &bytes,
            &backup_password,
        )?)
    })
    .await
    .map_err(|_| CmdError::internal())??;
    // Pulls first, then sends; an item deleted since the backup comes back,
    // a live one is never overwritten (havenkeys-client `restore`).
    let report = app
        .state::<AppState>()
        .client()
        .clone()
        .restore_backup(opened)
        .await?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Some(ImportResult { report, file_name }))
}
