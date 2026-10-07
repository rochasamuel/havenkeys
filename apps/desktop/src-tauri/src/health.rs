//! Vault health commands. The report holds item IDs and check kinds only;
//! help links are looked up in Rust and opened here, never passed in.

use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::health::{HealthCheck, HealthReport};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

#[tauri::command]
pub async fn health_report(app: AppHandle) -> CmdResult<HealthReport> {
    let state = app.state::<AppState>();
    state.touch();
    let client = state.client().clone();
    tauri::async_runtime::spawn_blocking(move || client.health_report(AppState::now_ms()))
        .await
        .map_err(|_| CmdError::internal())?
}

#[tauri::command]
pub async fn set_health_ignored(
    app: AppHandle,
    id: Uuid,
    checks: Vec<HealthCheck>,
) -> CmdResult<()> {
    let state = app.state::<AppState>();
    state.touch();
    let client = state.client().clone();
    client.set_health_ignored(id, checks).await
}

#[tauri::command]
pub fn open_health_help(state: State<'_, AppState>, id: Uuid, check: HealthCheck) -> CmdResult<()> {
    state.touch();
    let target = state.vault()?.health_help_url(&id, check)?;
    tauri_plugin_opener::open_url(target, None::<&str>).map_err(|_| CmdError::open_website())
}
