//! Import commands. The renderer never supplies a file path: Rust opens the
//! native file picker, reads the chosen file into memory, and remembers the
//! path only so the user can choose to delete the plaintext export afterwards.

use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::import::{onepux, ImportReport};
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

fn read_limited(path: &std::path::Path) -> CmdResult<Zeroizing<Vec<u8>>> {
    let meta = std::fs::metadata(path).map_err(|_| CmdError::file())?;
    if !meta.is_file() {
        return Err(CmdError::file());
    }
    if meta.len() > onepux::MAX_ARCHIVE_BYTES {
        return Err(havenkeys_core::Error::InvalidInput("export file is too large").into());
    }
    let file = std::fs::File::open(path).map_err(|_| CmdError::file())?;
    let mut buf = Zeroizing::new(Vec::with_capacity(meta.len() as usize));
    file.take(onepux::MAX_ARCHIVE_BYTES + 1)
        .read_to_end(&mut buf)
        .map_err(|_| CmdError::file())?;
    Ok(buf)
}

/// Let the user pick a 1Password `.1pux` export and import it.
/// Returns `None` if the picker was cancelled.
#[tauri::command]
pub async fn import_1pux(app: AppHandle) -> CmdResult<Option<ImportResult>> {
    let state = app.state::<AppState>();
    state.touch();
    if !state.vault()?.is_unlocked() {
        return Err(havenkeys_core::Error::Locked.into());
    }

    let handle = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        handle
            .dialog()
            .file()
            .set_title("Import from 1Password")
            .add_filter("1Password export", &["1pux"])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| CmdError::internal())?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|_| CmdError::file())?;

    // Read and parse off the async runtime; the vault lock is only taken to store.
    let parse_path = path.clone();
    let parsed = tauri::async_runtime::spawn_blocking(move || -> CmdResult<onepux::Parsed> {
        let bytes = read_limited(&parse_path)?;
        Ok(onepux::parse(&bytes)?)
    })
    .await
    .map_err(|_| CmdError::internal())??;

    let report = state
        .vault()?
        .import_items(parsed.items, parsed.report, AppState::now_ms())?;

    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Ok(mut last) = state.last_import.lock() {
        *last = Some(path);
    }
    Ok(Some(ImportResult { report, file_name }))
}

/// Delete the export file chosen in the last `import_1pux` call. This is a
/// normal file deletion, not a secure wipe (see docs/security-model.md).
#[tauri::command]
pub fn delete_import_file(state: tauri::State<'_, AppState>) -> CmdResult<()> {
    state.touch();
    let path = state
        .last_import
        .lock()
        .map_err(|_| CmdError::internal())?
        .take()
        .ok_or(havenkeys_core::Error::NotFound)?;
    std::fs::remove_file(&path).map_err(|_| CmdError::file())
}
