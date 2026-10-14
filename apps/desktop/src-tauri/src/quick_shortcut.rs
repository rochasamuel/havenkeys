//! The Quick Access shortcut (spec 2026-10-13-quick-access-design §2, §4):
//! registered and unregistered here from the setting in quick-access.json
//! (switch and chord, spec 2026-10-14-quick-access-shortcut-remap-design).
//! The plugin is added in Rust with a handler; no capability grants the
//! renderer its commands, so no webview can add or change a global shortcut.

use crate::quick_access::{
    QuickAccess, QuickAccessStatus, SetShortcutError, ShortcutHost, ShortcutRefusal, ShortcutTaken,
};
use crate::state::{AppState, CmdError, CmdResult};
use tauri::{AppHandle, Manager, State, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcut, Shortcut, ShortcutState};

/// Linux without an X display (a Wayland session with no XWayland) has no
/// global hotkeys; the plugin is then left out rather than failing the
/// app's start. Settings shows the shortcut as unavailable ("in use").
fn supported() -> bool {
    !cfg!(target_os = "linux") || std::env::var_os("DISPLAY").is_some()
}

/// Adds the plugin, with the one handler that toggles the quick window.
pub fn install(builder: tauri::Builder<Wry>) -> tauri::Builder<Wry> {
    if !supported() {
        return builder;
    }
    builder.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, pressed, event| {
                // Only the chord Quick Access answers to now (Settings may change it).
                if event.state() == ShortcutState::Pressed
                    && app
                        .try_state::<QuickAccess>()
                        .is_some_and(|qa| qa.is_current(pressed))
                {
                    crate::quick_window::toggle_from_shortcut(app);
                }
            })
            .build(),
    )
}

struct OsShortcuts<'a>(&'a AppHandle);

impl ShortcutHost for OsShortcuts<'_> {
    fn register(&self, shortcut: &Shortcut) -> Result<(), ShortcutTaken> {
        // No plugin (see `supported`): the shortcut cannot be had.
        let plugin = self
            .0
            .try_state::<GlobalShortcut<Wry>>()
            .ok_or(ShortcutTaken)?;
        // Every refusal reads as "in use". The plugin flattens global-hotkey's
        // errors into one string variant (`Error::GlobalHotkey(String)`), so
        // `AlreadyRegistered` (another app owns the chord) cannot be told
        // apart from an OS failure without matching on message text; either
        // way the shortcut is unavailable and the next `apply` retries it.
        plugin.register(*shortcut).map_err(|_| ShortcutTaken)
    }

    fn unregister(&self, shortcut: &Shortcut) {
        if let Some(plugin) = self.0.try_state::<GlobalShortcut<Wry>>() {
            let _ = plugin.unregister(*shortcut);
        }
    }
}

/// At startup, from the saved setting. On its own thread: the plugin's
/// register call waits for the main thread's event loop.
pub fn register_in_background(app: AppHandle) {
    let _ = std::thread::Builder::new()
        .name("quick-access".into())
        .spawn(move || {
            if let Some(qa) = app.try_state::<QuickAccess>() {
                qa.apply(&OsShortcuts(&app));
            }
        });
}

fn settings_failed() -> CmdError {
    CmdError {
        code: "quick_access_settings",
        message: "Could not save the Quick Access setting.".into(),
    }
}

/// One code, one fixed message per reason; the UI translates each
/// message (i18n `errors.shortcutRefused`).
fn shortcut_refused(reason: ShortcutRefusal) -> CmdError {
    match reason {
        ShortcutRefusal::Modifier => CmdError {
            code: "quick_access_shortcut_invalid",
            message: "Add Cmd, Ctrl or Alt.".into(),
        },
        ShortcutRefusal::Key => CmdError {
            code: "quick_access_shortcut_invalid",
            message: "This key can't be used.".into(),
        },
        ShortcutRefusal::Reserved => CmdError {
            code: "quick_access_shortcut_invalid",
            message: "Reserved by the system.".into(),
        },
    }
}

/// Not sensitive; readable locked or not. No `touch()`: Settings reads it
/// whenever it opens.
#[tauri::command]
pub fn quick_access_status(qa: State<'_, QuickAccess>) -> QuickAccessStatus {
    qa.status()
}

/// The Settings switch. Async so it runs off the main thread, where the
/// plugin's register/unregister call is dispatched and awaited.
#[tauri::command]
pub async fn set_quick_access(app: AppHandle, enabled: bool) -> CmdResult<QuickAccessStatus> {
    {
        let state = app.state::<AppState>();
        state.touch();
        state.require_unlocked()?;
    }
    let qa = app.state::<QuickAccess>();
    qa.set(&OsShortcuts(&app), enabled)
        .map_err(|_| settings_failed())
}

/// Settings → Change… / Reset. `keys` is the renderer's proposal in
/// `parse_keys` form, or `None` for the default; Rust checks it again
/// whatever the renderer did. Taken by another app is an answer
/// (`candidateInUse`), not an error. Async for the same reason as
/// `set_quick_access`.
#[tauri::command]
pub async fn set_quick_access_shortcut(
    app: AppHandle,
    keys: Option<String>,
) -> CmdResult<QuickAccessStatus> {
    {
        let state = app.state::<AppState>();
        state.touch();
        state.require_unlocked()?;
    }
    let qa = app.state::<QuickAccess>();
    qa.set_shortcut(&OsShortcuts(&app), keys.as_deref())
        .map_err(|e| match e {
            SetShortcutError::Refused(reason) => shortcut_refused(reason),
            SetShortcutError::Save(_) => settings_failed(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quick_access::ShortcutRefusal;

    #[test]
    fn each_refusal_has_its_own_fixed_message_under_one_code() {
        let all = [
            ShortcutRefusal::Modifier,
            ShortcutRefusal::Key,
            ShortcutRefusal::Reserved,
        ]
        .map(shortcut_refused);
        assert!(all
            .iter()
            .all(|e| e.code == "quick_access_shortcut_invalid"));
        assert_eq!(
            all.map(|e| e.message),
            [
                "Add Cmd, Ctrl or Alt.",
                "This key can't be used.",
                "Reserved by the system.",
            ]
        );
    }
}
