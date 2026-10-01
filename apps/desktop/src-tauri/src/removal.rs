//! Remove this device: a thin wrapper over `havenkeys_client`. The
//! `vault://removed` event is sent by `DesktopEvents`.

use crate::state::{AppState, CmdResult};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn remove_device(app: AppHandle, confirmation: String) -> CmdResult<()> {
    let client = app.state::<AppState>().client().clone();
    client.remove_device(confirmation).await
}
