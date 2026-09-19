//! System tray: HavenKeys keeps running in the tray when its window is closed.
//!
//! The tray never shows item data; the tooltip and menu are static text.

use crate::state::AppState;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

const MAIN_WINDOW: &str = "main";

/// Bring the main window back (from the tray or a minimized state).
pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn install(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open HavenKeys", true, None::<&str>)?;
    let lock = MenuItem::with_id(app, "lock", "Lock", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit HavenKeys", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&open, &lock, &PredefinedMenuItem::separator(app)?, &quit],
    )?;

    let mut builder = TrayIconBuilder::with_id("havenkeys")
        .tooltip("HavenKeys")
        .menu(&menu)
        // Left click opens the window (Windows/macOS). Linux AppIndicator
        // always shows the menu instead, which has "Open HavenKeys".
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main_window(app),
            "lock" => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.lock(app, "user");
                }
            }
            "quit" => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.lock(app, "exit");
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
