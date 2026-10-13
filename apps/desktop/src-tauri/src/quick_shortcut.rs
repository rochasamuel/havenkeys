//! The Quick Access shortcut (spec 2026-10-13-quick-access-design §2, §4):
//! registered and unregistered here from the setting in quick-access.json.
//! The plugin is added in Rust with a handler; no capability grants the
//! renderer its commands, so no webview can add or change a global shortcut.

use crate::quick_access::{QuickAccess, QuickAccessStatus, ShortcutHost, ShortcutTaken};
use crate::state::{AppState, CmdError, CmdResult};
use tauri::{AppHandle, Manager, State, Wry};
use tauri_plugin_global_shortcut::{Code, GlobalShortcut, Modifiers, Shortcut, ShortcutState};

/// `Cmd+Shift+Space` on macOS, `Ctrl+Shift+Space` elsewhere. Fixed in this
/// version.
pub fn shortcut() -> Shortcut {
    let primary = if cfg!(target_os = "macos") {
        Modifiers::SUPER
    } else {
        Modifiers::CONTROL
    };
    Shortcut::new(Some(primary | Modifiers::SHIFT), Code::Space)
}

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
                if event.state() == ShortcutState::Pressed && *pressed == shortcut() {
                    crate::quick_window::toggle_from_shortcut(app);
                }
            })
            .build(),
    )
}

struct OsShortcuts<'a>(&'a AppHandle);

impl ShortcutHost for OsShortcuts<'_> {
    fn register(&self) -> Result<(), ShortcutTaken> {
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
        plugin.register(shortcut()).map_err(|_| ShortcutTaken)
    }

    fn unregister(&self) {
        if let Some(plugin) = self.0.try_state::<GlobalShortcut<Wry>>() {
            let _ = plugin.unregister(shortcut());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shortcut_is_command_or_control_shift_space() {
        let (primary, other) = if cfg!(target_os = "macos") {
            (Modifiers::SUPER, Modifiers::CONTROL)
        } else {
            (Modifiers::CONTROL, Modifiers::SUPER)
        };
        assert_eq!(
            shortcut(),
            Shortcut::new(Some(primary | Modifiers::SHIFT), Code::Space)
        );
        // Exactly this platform's modifier plus Shift: never the other
        // platform's, never Alt.
        assert_eq!(shortcut().mods, primary | Modifiers::SHIFT);
        assert!(!shortcut().mods.intersects(other | Modifiers::ALT));
        assert_eq!(shortcut().key, Code::Space);
    }
}
