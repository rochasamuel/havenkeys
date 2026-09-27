//! System tray: HavenKeys keeps running in the tray when its window is closed.
//!
//! The tray never shows item data; the tooltip and menu are static text,
//! taken from a fixed table in the UI's language.

use crate::state::{AppState, CmdError, CmdResult};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

const MAIN_WINDOW: &str = "main";

/// The languages the UI speaks. A closed set: the renderer names one, and
/// anything else is refused rather than falling back silently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiLanguage {
    En,
    PtBr,
}

impl UiLanguage {
    pub fn parse(tag: &str) -> CmdResult<Self> {
        match tag {
            "en" => Ok(Self::En),
            "pt-BR" => Ok(Self::PtBr),
            _ => Err(CmdError {
                code: "unsupported_language",
                message: "Unsupported language.".into(),
            }),
        }
    }

    /// Labels for the menu items `open`, `lock` and `quit`, in that order.
    pub fn labels(self) -> [&'static str; 3] {
        match self {
            Self::En => ["Open HavenKeys", "Lock", "Quit HavenKeys"],
            Self::PtBr => ["Abrir HavenKeys", "Bloquear", "Sair do HavenKeys"],
        }
    }
}

/// The relabelable menu items, kept so the UI's language can be applied
/// after the tray is built.
struct TrayItems {
    open: MenuItem<Wry>,
    lock: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

/// Relabel the tray menu in the UI's language. Only `"en"` and `"pt-BR"`
/// are accepted.
#[tauri::command]
pub fn set_ui_language(app: AppHandle, lang: String) -> CmdResult<()> {
    let [open, lock, quit] = UiLanguage::parse(&lang)?.labels();
    if let Some(items) = app.try_state::<TrayItems>() {
        for (item, label) in [
            (&items.open, open),
            (&items.lock, lock),
            (&items.quit, quit),
        ] {
            item.set_text(label).map_err(|_| CmdError::internal())?;
        }
    }
    Ok(())
}

/// Show or hide the tray's "Update available" item. (Task 3.)
pub fn set_update_available(_app: &AppHandle, _available: bool) {}

/// Bring the main window back (from the tray or a minimized state).
pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn install(app: &tauri::App) -> tauri::Result<()> {
    // English until the UI reports its language (`set_ui_language`).
    let [open_label, lock_label, quit_label] = UiLanguage::En.labels();
    let open = MenuItem::with_id(app, "open", open_label, true, None::<&str>)?;
    let lock = MenuItem::with_id(app, "lock", lock_label, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit_label, true, None::<&str>)?;
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
    app.manage(TrayItems { open, lock, quit });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_two_ui_languages_are_accepted() {
        assert_eq!(UiLanguage::parse("en").unwrap(), UiLanguage::En);
        assert_eq!(UiLanguage::parse("pt-BR").unwrap(), UiLanguage::PtBr);
        for tag in [
            "", "EN", "pt", "pt-br", "pt-PT", "fr", "en-US", " en", "<b>",
        ] {
            let err = UiLanguage::parse(tag).unwrap_err();
            assert_eq!(err.code, "unsupported_language");
            assert_eq!(err.message, "Unsupported language.");
        }
    }

    #[test]
    fn the_tray_labels_come_from_the_fixed_table() {
        assert_eq!(
            UiLanguage::En.labels(),
            ["Open HavenKeys", "Lock", "Quit HavenKeys"]
        );
        assert_eq!(
            UiLanguage::PtBr.labels(),
            ["Abrir HavenKeys", "Bloquear", "Sair do HavenKeys"]
        );
    }
}
