//! HavenKeys desktop shell. Thin by design: all security logic lives in
//! `havenkeys-core`; this crate wires it to Tauri, the clipboard and timers.

#![forbid(unsafe_code)]

mod account;
mod clipboard;
mod commands;
mod device;
mod import;
mod state;
mod tray;

use havenkeys_bridge::Bridge;
use havenkeys_core::store::Store;
use havenkeys_core::vault::VaultService;
use havenkeys_protocol::endpoint::Endpoint;
use state::AppState;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::plugin::TauriPlugin;
use tauri::{Emitter, Manager, RunEvent, Runtime, Url, WindowEvent};

const VAULT_FILE: &str = "vault.sqlite3";
const AUTO_LOCK_TICK: Duration = Duration::from_secs(5);

/// Only the bundled app may be loaded in the webview. Everything else
/// (remote sites, file://, javascript:, data:) is refused.
fn is_app_url(url: &Url) -> bool {
    match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "http" | "https" => {
            url.host_str() == Some("tauri.localhost")
                || (cfg!(debug_assertions)
                    && url.host_str() == Some("localhost")
                    && url.port() == Some(1420))
        }
        _ => false,
    }
}

fn navigation_guard<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("navigation-guard")
        .on_navigation(|_webview, url| is_app_url(url))
        .build()
}

fn vault_path<R: Runtime>(app: &tauri::App<R>) -> Result<PathBuf, &'static str> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| "Could not determine the data directory.")?;
    std::fs::create_dir_all(&dir).map_err(|_| "Could not create the data directory.")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "Could not secure the data directory.")?;
    }
    Ok(dir.join(VAULT_FILE))
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(navigation_guard())
        // Used from Rust only (native file picker for imports). The capability
        // grants the renderer no dialog permissions.
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let path = vault_path(app)?;
            let store = Store::open(&path).map_err(|e| e.to_string())?;
            let device = device::Device::load(path.parent().ok_or("no data directory")?);
            let vault = Arc::new(Mutex::new(VaultService::new(store)));

            // Browser integration. The bridge locks through AppState so an
            // extension-initiated lock behaves exactly like any other.
            let lock_handle = app.handle().clone();
            let change_handle = app.handle().clone();
            let bridge = Bridge::with_change_hook(
                vault.clone(),
                move || {
                    if let Some(state) = lock_handle.try_state::<AppState>() {
                        state.lock(&lock_handle, "extension");
                    }
                },
                // A login saved from the browser: the item list must refresh
                // (the payload is empty; the UI re-reads the list itself).
                move || {
                    let _ = change_handle.emit(state::ITEMS_CHANGED_EVENT, ());
                },
            );
            app.manage(AppState::new(vault, bridge.clone(), device));
            // Failure (another instance running, unsafe socket directory)
            // disables browser integration but not the app.
            let _ = Endpoint::for_current_user().and_then(|ep| bridge.serve(&ep));

            tray::install(app)?;

            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("auto-lock".into())
                .spawn(move || {
                    // Lock with the OS session (screen lock) where the
                    // platform tells us; see havenkeys-oslock.
                    let mut session = havenkeys_oslock::SessionWatcher::new();
                    loop {
                        std::thread::sleep(AUTO_LOCK_TICK);
                        let state = handle.state::<AppState>();
                        if session.poll() {
                            state.lock(&handle, "screen_lock");
                        }
                        state.auto_lock_tick(&handle);
                    }
                })?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(
                event,
                WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed
            ) {
                let app = window.app_handle();
                if let Some(state) = app.try_state::<AppState>() {
                    state.lock(app, "exit");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::vault_status,
            commands::unlock_vault,
            commands::lock_vault,
            commands::change_master_password,
            commands::record_activity,
            commands::list_items,
            commands::get_item,
            commands::reveal_secret,
            commands::password_history,
            account::device_status,
            account::get_emergency_kit,
            commands::reveal_previous_password,
            commands::get_totp_code,
            commands::copy_secret,
            commands::create_item,
            commands::update_item,
            commands::delete_item,
            commands::generate_password,
            commands::copy_generated_password,
            commands::get_settings,
            commands::update_settings,
            import::import_1pux,
            import::delete_import_file,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build the HavenKeys application");

    app.run(|handle, event| {
        if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
            if let Some(state) = handle.try_state::<AppState>() {
                state.lock(handle, "exit");
            }
        }
    });
}
