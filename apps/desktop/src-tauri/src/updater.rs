//! In-app updates: the updater plugin, the four commands and the timer.
//! The decisions live in `updates.rs`.
//!
//! The renderer never touches the plugin. It asks for the status, asks for
//! a check, and asks to install what is on offer; Rust does the rest in a
//! fixed order: download, verify the signature (inside `download`), lock
//! the vault, install, restart. See
//! docs/superpowers/specs/2026-09-27-desktop-auto-update-design.md.

use crate::state::{AppState, CmdError, CmdResult};
use crate::updates::{installs_in_place, CheckOutcome, Machine, Phase, UpdateSettings};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

pub const STATUS_EVENT: &str = "updates://status";
/// Where a `.deb`/`.rpm` install sends the user. Fixed here, never taken
/// from the downloaded manifest.
pub const RELEASES_URL: &str = "https://github.com/rochasamuel/havenkeys/releases/latest";
const FIRST_CHECK: Duration = Duration::from_secs(5);
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

pub struct Updates {
    machine: Mutex<Machine>,
    /// The update found by the last check, kept for `install_update`.
    pending: Mutex<Option<Update>>,
    settings: Mutex<UpdateSettings>,
    dir: PathBuf,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    #[serde(flatten)]
    phase: Phase,
    auto_check: bool,
    can_install_in_place: bool,
    current_version: String,
}

impl Updates {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            machine: Mutex::new(Machine::new()),
            pending: Mutex::new(None),
            settings: Mutex::new(UpdateSettings::load(&dir)),
            dir,
        }
    }

    fn auto_check(&self) -> bool {
        self.settings.lock().map(|s| s.auto_check).unwrap_or(true)
    }

    fn status(&self, app: &AppHandle) -> UpdateStatus {
        let phase = self
            .machine
            .lock()
            .map(|m| m.phase().clone())
            .unwrap_or(Phase::Idle);
        UpdateStatus {
            phase,
            auto_check: self.auto_check(),
            can_install_in_place: installs_in_place(),
            current_version: app.package_info().version.to_string(),
        }
    }
}

fn no_update() -> CmdError {
    CmdError {
        code: "update_unavailable",
        message: "There is no update to install.".into(),
    }
}

fn update_failed() -> CmdError {
    CmdError {
        code: "update_failed",
        message: "The update could not be installed. Try again later.".into(),
    }
}

fn settings_failed() -> CmdError {
    CmdError {
        code: "update_settings",
        message: "Could not save the update setting.".into(),
    }
}

/// Tell the window and the tray where things stand.
fn publish(app: &AppHandle) -> UpdateStatus {
    let status = app.state::<Updates>().status(app);
    crate::tray::set_update_available(app, matches!(status.phase, Phase::Available { .. }));
    let _ = app.emit(STATUS_EVENT, status.clone());
    status
}

async fn fetch(app: &AppHandle) -> tauri_plugin_updater::Result<Option<Update>> {
    app.updater()?.check().await
}

/// One check. Overlapping calls collapse: a check that finds one already
/// running (or an install under way) does nothing.
pub async fn check(app: &AppHandle, manual: bool) {
    let updates = app.state::<Updates>();
    let started = updates
        .machine
        .lock()
        .map(|mut m| m.begin_check())
        .unwrap_or(false);
    if !started {
        return;
    }
    publish(app);
    // The plugin's error text may carry URLs; only the fact of failure is kept.
    let outcome = match fetch(app).await {
        Ok(Some(update)) => {
            let found = CheckOutcome::Found {
                version: update.version.clone(),
                notes: update.body.clone().unwrap_or_default(),
            };
            if let Ok(mut pending) = updates.pending.lock() {
                *pending = Some(update);
            }
            found
        }
        Ok(None) => {
            if let Ok(mut pending) = updates.pending.lock() {
                *pending = None;
            }
            CheckOutcome::UpToDate
        }
        Err(_) => CheckOutcome::Failed,
    };
    if let Ok(mut m) = updates.machine.lock() {
        m.finish_check(outcome, manual);
    }
    publish(app);
}

fn fail_install(app: &AppHandle) -> CmdError {
    if let Ok(mut m) = app.state::<Updates>().machine.lock() {
        m.install_failed();
    }
    publish(app);
    update_failed()
}

async fn install(app: &AppHandle) -> CmdResult<()> {
    if !installs_in_place() {
        return tauri_plugin_opener::open_url(RELEASES_URL, None::<&str>)
            .map_err(|_| CmdError::open_website());
    }
    let updates = app.state::<Updates>();
    updates
        .machine
        .lock()
        .map_err(|_| CmdError::internal())?
        .begin_install()
        .map_err(|_| no_update())?;
    let update = updates.pending.lock().ok().and_then(|mut p| p.take());
    let Some(update) = update else {
        return Err(fail_install(app));
    };
    publish(app);

    let progress = app.clone();
    let bytes = update
        .download(
            move |chunk, total| {
                let changed = progress
                    .state::<Updates>()
                    .machine
                    .lock()
                    .map(|mut m| m.progress(chunk as u64, total))
                    .unwrap_or(false);
                if changed {
                    publish(&progress);
                }
            },
            || {},
        )
        .await;
    // `download` returns only bytes whose signature matched the public key
    // in tauri.conf.json. Anything else stops here, with the vault as it was.
    let Ok(bytes) = bytes else {
        return Err(fail_install(app));
    };

    app.state::<AppState>().lock(app, "update");
    if let Ok(mut m) = updates.machine.lock() {
        m.installing();
    }
    publish(app);
    // On Windows this starts the installer and ends the process.
    if update.install(bytes).is_err() {
        return Err(fail_install(app));
    }
    app.restart()
}

/// The automatic check: once shortly after start, then daily, while the
/// setting is on. Debug builds (`tauri dev`) never check on their own.
pub fn schedule(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            if app.state::<Updates>().auto_check() {
                check(&app, false).await;
            }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

#[tauri::command]
pub fn update_status(app: AppHandle, updates: State<'_, Updates>) -> CmdResult<UpdateStatus> {
    Ok(updates.status(&app))
}

/// "Check now". Failure shows up in the returned status, not as an error.
#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> CmdResult<UpdateStatus> {
    check(&app, true).await;
    Ok(app.state::<Updates>().status(&app))
}

/// Install what the last check found, or (a `.deb`/`.rpm` install) open
/// the release page. Allowed while locked: the banner shows on the unlock
/// screen too, and installing locks anyway.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> CmdResult<()> {
    install(&app).await
}

/// Changing it requires an unlocked vault, like every other setting.
#[tauri::command]
pub fn set_update_auto_check(
    app: AppHandle,
    state: State<'_, AppState>,
    updates: State<'_, Updates>,
    enabled: bool,
) -> CmdResult<UpdateStatus> {
    state.touch();
    if !state.vault()?.is_unlocked() {
        return Err(havenkeys_core::Error::Locked.into());
    }
    let next = UpdateSettings { auto_check: enabled };
    // Saved first: if the file cannot be written, nothing changes.
    next.save(&updates.dir).map_err(|_| settings_failed())?;
    *updates.settings.lock().map_err(|_| CmdError::internal())? = next;
    if enabled {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move { check(&handle, false).await });
    }
    Ok(publish(&app))
}

#[cfg(test)]
mod tests {
    /// The shipped updater config, read the way the plugin reads it.
    fn shipped_config() -> tauri_plugin_updater::Config {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        serde_json::from_value(conf["plugins"]["updater"].clone()).unwrap()
    }

    #[test]
    fn the_updater_refuses_downgrades_and_unsigned_versions() {
        let config = shipped_config();
        // latest.json is not signed; only the signature binds the version.
        assert!(config.require_signed_version);
        assert!(!config.allow_downgrades);
        assert!(!config.dangerous_insecure_transport_protocol);
        assert!(!config.dangerous_accept_invalid_certs);
        assert!(!config.dangerous_accept_invalid_hostnames);
        assert!(!config.endpoints.is_empty());
        assert!(config.endpoints.iter().all(|url| url.scheme() == "https"));
    }
}
