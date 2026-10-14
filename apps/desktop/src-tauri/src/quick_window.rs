//! The Quick Access window (spec 2026-10-13-quick-access-design §3): shown
//! and hidden here, never navigated anywhere but the bundled quick.html.
//! The decisions are in quick_access.rs.

use crate::quick_access::{
    self, HideReason, QuickAccess, QuickSurface, ShortcutTarget, Toggled, QUICK_WINDOW, SHOWN_EVENT,
};
use crate::state::AppState;
use crate::tray::{self, MAIN_WINDOW};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow, Window, WindowEvent};

struct Surface<'a> {
    app: &'a AppHandle,
    window: WebviewWindow,
}

impl QuickSurface for Surface<'_> {
    fn is_visible(&self) -> bool {
        self.window.is_visible().unwrap_or(false)
    }

    fn show_focused(&self) {
        let main_focused = self
            .app
            .get_webview_window(MAIN_WINDOW)
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false);
        if let Some(qa) = self.app.try_state::<QuickAccess>() {
            qa.note_opened(main_focused);
        }
        center_on_cursor_display(&self.window);
        let _ = self.window.show();
        let _ = self.window.set_focus();
        let _ = self.app.emit_to(QUICK_WINDOW, SHOWN_EVENT, ());
    }

    fn hide(&self) {
        let _ = self.window.hide();
    }
}

/// Centre on the display the pointer is on (the one the user is looking
/// at), falling back to the window's own display, then the primary one.
fn center_on_cursor_display(window: &WebviewWindow) {
    let monitor = window
        .cursor_position()
        .ok()
        .and_then(|p| window.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    let (Some(monitor), Ok(size)) = (monitor, window.outer_size()) else {
        let _ = window.center();
        return;
    };
    let (pos, msize) = (monitor.position(), monitor.size());
    let (x, y) = quick_access::centered(
        (pos.x, pos.y),
        (msize.width, msize.height),
        (size.width, size.height),
    );
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// The global shortcut was pressed.
pub fn toggle_from_shortcut(app: &AppHandle) {
    let vault_exists = app
        .try_state::<AppState>()
        .and_then(|s| s.vault().ok().and_then(|v| v.status().ok()))
        .map(|s| s.vault_exists);
    if quick_access::shortcut_target(vault_exists) == ShortcutTarget::Main {
        tray::show_main_window(app);
        return;
    }
    let Some(window) = app.get_webview_window(QUICK_WINDOW) else {
        return;
    };
    if quick_access::toggle(&Surface { app, window }) == Toggled::Hidden {
        return_focus(app, HideReason::Dismissed);
    }
}

/// Hide the quick window if it exists, then hand the keyboard back as
/// `reason` calls for (macOS: hides the app when focus came from another
/// app). Runs the focus step even when the window was already hidden.
pub fn hide(app: &AppHandle, reason: HideReason) {
    if let Some(window) = app.get_webview_window(QUICK_WINDOW) {
        let _ = window.hide();
    }
    return_focus(app, reason);
}

#[cfg(target_os = "macos")]
fn return_focus(app: &AppHandle, reason: HideReason) {
    let main_was_focused = app
        .try_state::<QuickAccess>()
        .map(|qa| qa.main_was_focused())
        .unwrap_or(true);
    if quick_access::returns_focus_to_previous_app(main_was_focused, reason) {
        let _ = app.hide();
    }
}

/// Windows and X11 give the keyboard back to the previous window by
/// themselves when this one hides.
#[cfg(not(target_os = "macos"))]
fn return_focus(_app: &AppHandle, _reason: HideReason) {}

/// Esc or an action in the quick window. Takes nothing from the renderer
/// and can only hide the quick window.
#[tauri::command]
pub fn hide_quick_window(app: AppHandle) {
    hide(&app, HideReason::Dismissed);
}

/// The quick window's own events. Its close button (Alt+F4) hides it like
/// Esc; it never locks the vault or quits the app.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide(window.app_handle(), HideReason::Dismissed);
        }
        WindowEvent::Focused(false) => hide(window.app_handle(), HideReason::Blurred),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    /// Spec §4: the quick window may call exactly these, and nothing else.
    const QUICK_PERMISSIONS: &[&str] = &[
        "core:event:allow-listen",
        "core:event:allow-unlisten",
        "allow-vault-status",
        "allow-unlock-vault",
        "allow-record-activity",
        "allow-list-items",
        "allow-copy-secret",
        "allow-open-website",
        "allow-hide-quick-window",
    ];

    fn json(text: &str) -> serde_json::Value {
        serde_json::from_str(text).unwrap()
    }

    fn strings(v: &serde_json::Value) -> Vec<String> {
        let mut out: Vec<String> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap().to_owned())
            .collect();
        out.sort();
        out
    }

    #[test]
    fn the_quick_capability_grants_exactly_the_allowed_list() {
        let cap = json(include_str!("../capabilities/quick.json"));
        assert_eq!(cap["identifier"], "quick");
        assert_eq!(strings(&cap["windows"]), ["quick"]);
        let mut want: Vec<String> = QUICK_PERMISSIONS.iter().map(|s| (*s).to_owned()).collect();
        want.sort();
        assert_eq!(strings(&cap["permissions"]), want);
    }

    #[test]
    fn the_main_capability_is_only_for_the_main_window() {
        let cap = json(include_str!("../capabilities/main.json"));
        assert_eq!(strings(&cap["windows"]), ["main"]);
        let granted = strings(&cap["permissions"]);
        assert!(!granted.iter().any(|p| p == "allow-hide-quick-window"));
        // Choosing the chord is a Settings action: main window only (the
        // quick window's exact list is pinned elsewhere).
        assert!(granted
            .iter()
            .any(|p| p == "allow-set-quick-access-shortcut"));
        // Neither window may drive the shortcut plugin (spec §4).
        let quick = json(include_str!("../capabilities/quick.json"));
        for p in granted.iter().chain(strings(&quick["permissions"]).iter()) {
            assert!(!p.starts_with("global-shortcut:"), "{p}");
        }
    }

    #[test]
    fn the_quick_window_is_declared_hidden_local_and_undecorated() {
        let conf = json(include_str!("../tauri.conf.json"));
        let windows = conf["app"]["windows"].as_array().unwrap();
        let quick = windows.iter().find(|w| w["label"] == "quick").unwrap();
        assert_eq!(quick["url"], "quick.html");
        assert_eq!(quick["visible"], false);
        assert_eq!(quick["decorations"], false);
        assert_eq!(quick["alwaysOnTop"], true);
        assert_eq!(quick["skipTaskbar"], true);
        assert_eq!(quick["resizable"], false);
        assert_eq!(
            strings(&conf["app"]["security"]["capabilities"]),
            ["main", "quick"]
        );
        // One CSP for both windows: there is no per-window override.
        assert!(quick.get("csp").is_none());
    }
}
