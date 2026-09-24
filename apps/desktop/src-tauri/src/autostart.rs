//! Open HavenKeys when the user logs in to the computer.
//!
//! This is a setting of this computer, not of the vault: it is stored by the
//! OS (a login item on macOS, `HKCU\…\Run` on Windows, an XDG autostart
//! entry on Linux) through `tauri-plugin-autostart`, and the OS is the only
//! record of it. Nothing is written to the vault or synced.
//!
//! A login launch passes [`LOGIN_ARG`] and starts in the tray, locked, with
//! no window: the user opens it when they need it. The plugin's own JS
//! commands are not granted by the capability, so the renderer reaches this
//! only through the two commands below.

use crate::state::{AppState, CmdError, CmdResult};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

/// The argument the OS login entry starts HavenKeys with.
pub const LOGIN_ARG: &str = "--autostart";

/// The plugin, registered with the argument that marks a login launch.
pub fn plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_autostart::Builder::new()
        .arg(LOGIN_ARG)
        .build()
}

/// Whether this process was started by the OS login entry.
pub fn launched_at_login() -> bool {
    std::env::args_os().skip(1).any(|a| a == LOGIN_ARG)
}

fn unavailable() -> CmdError {
    CmdError {
        code: "autostart",
        message: "Could not change whether HavenKeys opens at login.".into(),
    }
}

#[tauri::command]
pub fn launch_at_login(app: AppHandle) -> CmdResult<bool> {
    app.autolaunch().is_enabled().map_err(|_| unavailable())
}

/// Changing it requires an unlocked vault, like every other setting.
#[tauri::command]
pub fn set_launch_at_login(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> CmdResult<bool> {
    state.touch();
    if !state.vault()?.is_unlocked() {
        return Err(havenkeys_core::Error::Locked.into());
    }
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    result.map_err(|_| unavailable())?;
    manager.is_enabled().map_err(|_| unavailable())
}
