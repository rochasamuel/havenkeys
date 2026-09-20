//! Import commands.
//!
//! Storing imported items needs a server to accept the writes (spec
//! 2026-09-20 §8.4); it is rebuilt on top of staged writes once the sync
//! client exists (§13). Until then this refuses before ever touching the
//! file picker or the vault.

use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::import::ImportReport;
use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    report: ImportReport,
    /// File name only (no directory), for the confirmation message.
    file_name: String,
}

/// Let the user pick a 1Password `.1pux` export and import it.
/// Returns `None` if the picker was cancelled.
#[tauri::command]
pub async fn import_1pux(app: AppHandle) -> CmdResult<Option<ImportResult>> {
    let state = app.state::<AppState>();
    state.touch();
    // Writes need a server session (spec 2026-09-20 §8.4); Task 7 gates this
    // on the connectivity state once there is one to be in.
    Err(havenkeys_core::Error::Offline.into())
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
