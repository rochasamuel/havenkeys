//! HavenKeys desktop shell. Thin by design: all security logic lives in
//! `havenkeys-core`; this crate wires it to Tauri, the clipboard and timers.

#![forbid(unsafe_code)]

mod account;
mod autostart;
mod card;
mod clipboard;
mod commands;
mod custom_field;
mod emergency_kit;
mod events;
mod identity;
mod import;
mod item_input;
mod native_host;
mod qr_scan;
mod removal;
mod scan_slot;
mod secret_store;
mod state;
mod sync;
mod tray;
mod updater;
mod updates;

use havenkeys_bridge::Bridge;
use havenkeys_core::store::Store;
use havenkeys_core::vault::VaultService;
use havenkeys_protocol::endpoint::Endpoint;
use state::AppState;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Emitter, Manager, RunEvent, Runtime, Url, WindowEvent};

pub(crate) const VAULT_FILE: &str = "vault.sqlite3";
const AUTO_LOCK_TICK: Duration = Duration::from_secs(5);
use havenkeys_client::PULL_INTERVAL;

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

/// Open the vault file. A vault this build cannot open must not take the app
/// down with it: a panic in the setup hook leaves the user with a stack
/// trace in a terminal they may not even be looking at. The app opens on an
/// empty in-memory store instead, every command refuses with the returned
/// reason, and the window says what to do.
fn open_store(path: &Path, dir: &Path) -> Result<(Store, Option<state::CmdError>), String> {
    match Store::open(path) {
        Ok(store) => Ok((store, None)),
        Err(_) => Ok((
            Store::open_in_memory().map_err(|e| e.to_string())?,
            Some(state::CmdError::vault_unreadable(dir)),
        )),
    }
}

/// Browser integration. The bridge locks through AppState so an
/// extension-initiated lock behaves exactly like any other.
fn browser_bridge(app: &AppHandle, vault: Arc<Mutex<VaultService>>) -> Bridge {
    let lock_handle = app.clone();
    let change_handle = app.clone();
    let save_handle = app.clone();
    let bridge = Bridge::with_writer(
        vault,
        move || {
            if let Some(state) = lock_handle.try_state::<AppState>() {
                state.lock("extension");
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
            let Some(state) = save_handle.try_state::<AppState>() else {
                return Err(havenkeys_protocol::ErrorCode::Internal);
            };
            let client = state.client().clone();
            tauri::async_runtime::block_on(async move { client.push(staged).await.map(|_| ()) })
                .map_err(sync::bridge_error)
        },
    );
    // "Edit in HavenKeys" from the extension popup. The bridge has
    // already checked the item is saved for the page the popup was
    // opened on; this only raises the window and tells the UI.
    let open_handle = app.clone();
    bridge.set_open_item_hook(move |id| {
        tray::show_main_window(&open_handle);
        let _ = open_handle.emit(state::OPEN_ITEM_EVENT, id.to_string());
    });
    bridge
}

/// A Secret Key an earlier version (or a keychain that was unavailable
/// then) left in device.json moves to the keychain. Off the setup path: it
/// may wait on the keychain.
fn migrate_secret_key_in_background(app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("keychain-migrate".into())
        .spawn(move || {
            let state = app.state::<AppState>();
            // Vault before device; the vault guard ends here.
            let account = state.vault().ok().and_then(|v| v.account().ok().flatten());
            if let Some(account) = account {
                if let Ok(mut device) = state.client().device() {
                    device.migrate(account.account_id);
                }
            }
        });
}

/// The auto-lock thread: every tick it locks with the OS session and the
/// idle timer, and pulls from the server when a pull is due.
fn start_auto_lock(handle: AppHandle) -> std::io::Result<()> {
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
                    state.lock("screen_lock");
                }
                state.auto_lock_tick();
                // Catch up with the server while unlocked. A locked
                // vault has no session, so this simply does not run.
                if state.is_online() && state.client().sync_due(PULL_INTERVAL) {
                    let client = state.client().clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = client.sync_now().await;
                    });
                }
            }
        })?;
    Ok(())
}

pub fn run() {
    let app = tauri::Builder::default()
        // First, so a second launch (the app icon while the login launch
        // sits in the tray) hands over to this process and exits before it
        // opens the vault or the bridge.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main_window(app);
        }))
        .plugin(navigation_guard())
        // Used from Rust only (autostart.rs). The capability grants the
        // renderer none of the plugin's commands.
        .plugin(autostart::plugin())
        // Used from Rust only (native file picker for imports). The capability
        // grants the renderer no dialog permissions.
        .plugin(tauri_plugin_dialog::init())
        // Used from Rust only (updater.rs). The capability grants the
        // renderer none of the plugin's commands.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let path = vault_path(app)?;
            let dir = path.parent().ok_or("no data directory")?.to_path_buf();
            let (store, storage_error) = open_store(&path, &dir)?;
            let vault = Arc::new(Mutex::new(VaultService::new(store)));
            // Bounded by the keychain timeout; `Device::load` itself reads
            // only device.json.
            let device = havenkeys_client::device::Device::load(
                &dir,
                Box::new(secret_store::OsKeyStore::install()),
            );
            let events = Arc::new(events::DesktopEvents {
                app: app.handle().clone(),
            });
            let client = havenkeys_client::HavenClient::new(
                vault.clone(),
                device,
                storage_error,
                events,
                havenkeys_client::ClientConfig {
                    device_name: "Desktop",
                    vault_path: path.clone(),
                },
            );
            let bridge = browser_bridge(app.handle(), vault.clone());
            app.manage(AppState::new(client, vault, bridge.clone()));
            migrate_secret_key_in_background(app.handle().clone());
            // Failure (another instance running, unsafe socket directory)
            // disables browser integration but not the app.
            let _ = Endpoint::for_current_user().and_then(|ep| bridge.serve(&ep));
            // Point the browsers at the bundled native host (native_host.rs).
            native_host::register_in_background(dir.clone());

            tray::install(app)?;

            // In-app updates: this computer's setting and the daily check.
            app.manage(updater::Updates::new(dir.clone()));
            updater::schedule(app.handle().clone());

            // The window starts hidden (tauri.conf.json). A login launch
            // stays in the tray; any other launch shows it.
            if !autostart::launched_at_login() {
                tray::show_main_window(app.handle());
            }

            start_auto_lock(app.handle().clone())?;
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
                    state.lock("exit");
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
            commands::list_passkeys,
            commands::delete_passkey,
            account::device_status,
            account::account_status,
            account::activate_account,
            account::sign_in,
            account::sign_out,
            account::list_devices,
            account::revoke_device,
            removal::remove_device,
            emergency_kit::get_emergency_kit,
            account::reveal_account_secret_key,
            account::copy_account_field,
            identity::identity_item_id,
            identity::reveal_identity,
            identity::copy_identity_field,
            card::reveal_card,
            card::reveal_card_field,
            card::copy_card_field,
            card::check_card_number,
            custom_field::login_fields,
            custom_field::reveal_login_field,
            custom_field::login_field_totp,
            custom_field::copy_login_field,
            custom_field::open_login_field_url,
            commands::sync_now,
            commands::resync_vault,
            commands::reveal_previous_password,
            commands::get_totp_code,
            commands::copy_secret,
            commands::open_website,
            commands::scan_totp_qr,
            commands::create_item,
            commands::update_item,
            commands::delete_item,
            commands::generate_password,
            commands::copy_generated_password,
            commands::get_settings,
            commands::update_settings,
            autostart::launch_at_login,
            autostart::set_launch_at_login,
            import::import_1pux,
            import::delete_import_file,
            tray::set_ui_language,
            updater::update_status,
            updater::check_for_update,
            updater::install_update,
            updater::set_update_auto_check,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build the HavenKeys application");

    app.run(|handle, event| {
        if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
            if let Some(state) = handle.try_state::<AppState>() {
                state.lock("exit");
            }
        }
    });
}
