//! HavenKeys desktop shell. Thin by design: all security logic lives in
//! `havenkeys-core`; this crate wires it to Tauri, the clipboard and timers.

#![forbid(unsafe_code)]

mod account;
mod clipboard;
mod commands;
mod device;
mod import;
mod state;
mod sync;
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
const PULL_INTERVAL: Duration = Duration::from_secs(sync::PULL_INTERVAL_SECS);

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
            let dir = path.parent().ok_or("no data directory")?.to_path_buf();
            // A vault this build cannot open must not take the app down with
            // it: a panic in the setup hook leaves the user with a stack
            // trace in a terminal they may not even be looking at. The app
            // opens on an empty in-memory store, every command refuses with
            // the reason, and the window says what to do.
            let (store, storage_error) = match Store::open(&path) {
                Ok(store) => (store, None),
                Err(_) => (
                    Store::open_in_memory().map_err(|e| e.to_string())?,
                    Some(state::CmdError::vault_unreadable(&dir)),
                ),
            };
            let device = device::Device::load(&dir);
            let vault = Arc::new(Mutex::new(VaultService::new(store)));

            // Browser integration. The bridge locks through AppState so an
            // extension-initiated lock behaves exactly like any other.
            let lock_handle = app.handle().clone();
            let change_handle = app.handle().clone();
            let save_handle = app.handle().clone();
            let bridge = Bridge::with_writer(
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
                // A save from the browser is an ordinary write: the server
                // assigns the revision, and only then is it recorded here.
                // The bridge thread waits for it, without the vault lock.
                move |staged| {
                    let handle = save_handle.clone();
                    tauri::async_runtime::block_on(async move {
                        sync::push(&handle, staged).await.map(|_| ())
                    })
                    .map_err(sync::bridge_error)
                },
            );
            app.manage(AppState::new(vault, bridge.clone(), device, storage_error));
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
                        // Catch up with the server while unlocked. A locked
                        // vault has no session, so this simply does not run.
                        if state.is_online() && state.sync_due(PULL_INTERVAL) {
                            let pull_handle = handle.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = sync::sync_now(&pull_handle).await;
                            });
                        }
                    }
                })?;
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Closing the window hides HavenKeys to the tray instead of
            // quitting: the tray menu's "Open HavenKeys" brings it back, and
            // "Quit HavenKeys" is the way out. The vault is deliberately not
            // locked here — the idle timer keeps running while the window is
            // hidden, so auto-lock still applies (`security-model.md` §5).
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            // The window is actually going away (quit, or the session ending).
            WindowEvent::Destroyed => {
                let app = window.app_handle();
                if let Some(state) = app.try_state::<AppState>() {
                    state.lock(app, "exit");
                }
            }
            _ => {}
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
            account::account_status,
            account::activate_account,
            account::sign_in,
            account::sign_out,
            account::list_devices,
            account::revoke_device,
            account::get_emergency_kit,
            commands::sync_now,
            commands::resync_vault,
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
