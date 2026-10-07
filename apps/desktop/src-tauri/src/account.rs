//! Account commands: thin wrappers over `havenkeys_client`.

use crate::commands::{copy_to_clipboard, CopyResult};
use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_client::{AccountField, AccountStatus, DeviceEntry, DeviceStatus};
use havenkeys_core::vault::VaultStatus;
use havenkeys_core::SecretString;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

/// Run `f` on a blocking thread. For commands that touch the keychain
/// (through `Device`): a sync command runs on the main thread, where a
/// keychain waiting on D-Bus or a prompt would freeze the window.
pub(crate) async fn off_main_thread<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&AppState) -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>()))
        .await
        .map_err(|_| CmdError::internal())?
}

/// Safe to call while locked: reveals no secrets.
#[tauri::command]
pub async fn device_status(app: AppHandle) -> CmdResult<DeviceStatus> {
    off_main_thread(app, |state| state.client().device_status()).await
}

/// The account this vault belongs to. No secrets; safe while locked.
#[tauri::command]
pub fn account_status(state: State<'_, AppState>) -> CmdResult<Option<AccountStatus>> {
    state.client().account_status()
}

/// First run: the user pastes the invite and chooses a master password.
#[tauri::command]
pub async fn activate_account(
    app: AppHandle,
    invite: String,
    password: SecretString,
) -> CmdResult<VaultStatus> {
    let client = app.state::<AppState>().client().clone();
    client.activate(invite, password).await
}

/// A second device: server, email, master password and the Secret Key from
/// the Recovery Sheet.
#[tauri::command]
pub async fn sign_in(
    app: AppHandle,
    server_url: String,
    email: String,
    password: SecretString,
    secret_key: Option<SecretString>,
) -> CmdResult<VaultStatus> {
    let client = app.state::<AppState>().client().clone();
    client
        .sign_in(server_url, email, password, secret_key)
        .await
}

#[tauri::command]
pub async fn list_devices(app: AppHandle) -> CmdResult<Vec<DeviceEntry>> {
    let state = app.state::<AppState>();
    state.touch();
    let client = state.client().clone();
    client.list_devices().await
}

/// Cut a device off. Revoking this one signs it out immediately.
#[tauri::command]
pub async fn revoke_device(app: AppHandle, id: Uuid) -> CmdResult<()> {
    let state = app.state::<AppState>();
    state.touch();
    let client = state.client().clone();
    client.revoke_device(id).await
}

/// End this device's session and lock the vault.
#[tauri::command]
pub async fn sign_out(app: AppHandle) -> CmdResult<()> {
    let client = app.state::<AppState>().client().clone();
    client.sign_out().await
}

/// The Secret Key, on an explicit reveal from the Account item.
#[tauri::command]
pub async fn reveal_account_secret_key(app: AppHandle) -> CmdResult<SecretString> {
    off_main_thread(app, |state| {
        state.touch();
        state
            .client()
            .account_value(AccountField::SecretKey)
            .map(|(value, _)| value)
    })
    .await
}

/// Copy one of the Account item's values. It goes from here to the
/// clipboard, cleared after the usual delay; the renderer never holds it.
#[tauri::command]
pub async fn copy_account_field(app: AppHandle, field: AccountField) -> CmdResult<CopyResult> {
    off_main_thread(app, move |state| {
        state.touch();
        let (value, seconds) = state.client().account_value(field)?;
        copy_to_clipboard(state, &value, seconds)
    })
    .await
}
