//! Remove this device, or delete the whole account: thin wrappers over
//! `havenkeys_client`. The `vault://removed` and `vault://account-deleted`
//! events are sent by `DesktopEvents`.

use crate::state::{AppState, CmdResult};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn remove_device(app: AppHandle, confirmation: String) -> CmdResult<()> {
    let client = app.state::<AppState>().client().clone();
    client.remove_device(confirmation).await
}

/// Delete the account on the server, then this computer's copy. The client
/// checks unlocked, online, the typed email and the master password.
#[tauri::command]
pub async fn delete_account(
    app: AppHandle,
    confirmation: String,
    master_password: havenkeys_core::SecretString,
) -> CmdResult<()> {
    let state = app.state::<AppState>();
    state.touch();
    let client = state.client().clone();
    client.delete_account(confirmation, master_password).await
}
