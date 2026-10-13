//! Quick Access: the global shortcut and the small search window (spec
//! docs/superpowers/specs/2026-10-13-quick-access-design.md).
//!
//! The decisions live here, behind two small traits so the tests need no
//! OS hook and no window: `ShortcutHost` registers the one fixed shortcut
//! (quick_shortcut.rs), `QuickSurface` shows and hides the window
//! (quick_window.rs).
// Used by quick_window.rs and quick_shortcut.rs (later tasks of the plan).
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub const QUICK_WINDOW: &str = "quick";
/// Sent to the quick window each time it is shown: it re-reads the lock
/// state, starts empty and focuses its field. Carries nothing.
pub const SHOWN_EVENT: &str = "quick://shown";
const FILE: &str = "quick-access.json";
const TMP: &str = "quick-access.json.tmp";

/// This computer's choice, outside the vault: the shortcut must work while
/// the vault is locked (to unlock from Quick Access), and the vault's
/// settings are sealed under the vault key. Holds nothing secret
/// (security-model.md §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickAccessSetting {
    #[serde(default = "default_on")]
    pub shortcut: bool,
}

fn default_on() -> bool {
    true
}

impl Default for QuickAccessSetting {
    fn default() -> Self {
        Self { shortcut: true }
    }
}

impl QuickAccessSetting {
    /// Missing, unreadable or malformed means on. Unknown fields (a newer
    /// version's) are ignored rather than rejected.
    pub fn load(dir: &Path) -> Self {
        std::fs::read(dir.join(FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Written to a temporary file and renamed over the old one.
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let bytes = serde_json::to_vec(self).map_err(std::io::Error::other)?;
        let tmp = dir.join(TMP);
        // A stale file from a crash may have looser permissions.
        let _ = std::fs::remove_file(&tmp);
        {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&tmp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        std::fs::rename(&tmp, dir.join(FILE))
    }
}

/// The OS refused the shortcut: another app owns it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShortcutTaken;

/// Registers the one fixed shortcut with the OS. Implemented over the
/// plugin in quick_shortcut.rs; faked in the tests.
pub trait ShortcutHost {
    fn register(&self) -> Result<(), ShortcutTaken>;
    fn unregister(&self);
}

/// What Settings shows: the switch, and whether the OS refused it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickAccessStatus {
    pub enabled: bool,
    pub in_use: bool,
}

#[derive(Clone, Copy, Default)]
struct Registration {
    enabled: bool,
    registered: bool,
    in_use: bool,
}

pub struct QuickAccess {
    dir: PathBuf,
    registration: Mutex<Registration>,
    /// Serializes `apply`. Held across the host's register/unregister call,
    /// which waits for the main thread; `registration` never is, so
    /// `status()` (a sync command, on the main thread) cannot deadlock
    /// against it.
    applying: Mutex<()>,
    /// Whether the main window had the keyboard when the quick window
    /// opened (macOS: hiding the app returns focus to the previous app).
    main_was_focused: AtomicBool,
}

impl QuickAccess {
    pub fn new(dir: PathBuf) -> Self {
        let enabled = QuickAccessSetting::load(&dir).shortcut;
        Self {
            dir,
            registration: Mutex::new(Registration {
                enabled,
                ..Registration::default()
            }),
            applying: Mutex::new(()),
            main_was_focused: AtomicBool::new(false),
        }
    }

    fn snapshot(&self) -> Registration {
        *self.registration.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn status(&self) -> QuickAccessStatus {
        let r = self.snapshot();
        QuickAccessStatus {
            enabled: r.enabled,
            in_use: r.enabled && r.in_use,
        }
    }

    /// Bring the OS registration in line with the setting. Idempotent:
    /// registers only when on and not yet registered (so a refused
    /// shortcut is retried by the next call), unregisters only when off and
    /// registered.
    pub fn apply(&self, host: &impl ShortcutHost) -> QuickAccessStatus {
        let _one_at_a_time = self.applying.lock().unwrap_or_else(|p| p.into_inner());
        self.apply_locked(host)
    }

    /// `applying` must be held by the caller.
    fn apply_locked(&self, host: &impl ShortcutHost) -> QuickAccessStatus {
        let r = self.snapshot();
        let next = if r.enabled && !r.registered {
            match host.register() {
                Ok(()) => Registration {
                    registered: true,
                    in_use: false,
                    ..r
                },
                Err(ShortcutTaken) => Registration {
                    registered: false,
                    in_use: true,
                    ..r
                },
            }
        } else if !r.enabled && r.registered {
            host.unregister();
            Registration {
                registered: false,
                in_use: false,
                ..r
            }
        } else if !r.enabled {
            Registration { in_use: false, ..r }
        } else {
            r
        };
        // Only what this call changed; `enabled` is never written back.
        {
            let mut current = self.registration.lock().unwrap_or_else(|p| p.into_inner());
            current.registered = next.registered;
            current.in_use = next.in_use;
        }
        self.status()
    }

    /// The Settings switch: saved first, so if the file cannot be written
    /// nothing changes; then registered or unregistered at once.
    pub fn set(
        &self,
        host: &impl ShortcutHost,
        enabled: bool,
    ) -> std::io::Result<QuickAccessStatus> {
        // Held across save, enable and apply, so two sets cannot interleave.
        let _one_at_a_time = self.applying.lock().unwrap_or_else(|p| p.into_inner());
        QuickAccessSetting { shortcut: enabled }.save(&self.dir)?;
        self.registration
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .enabled = enabled;
        Ok(self.apply_locked(host))
    }

    pub fn note_opened(&self, main_was_focused: bool) {
        self.main_was_focused
            .store(main_was_focused, Ordering::Relaxed);
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn main_was_focused(&self) -> bool {
        self.main_was_focused.load(Ordering::Relaxed)
    }
}

/// The quick window as the toggle sees it. Implemented over the Tauri
/// window in quick_window.rs; faked in the tests.
pub trait QuickSurface {
    fn is_visible(&self) -> bool;
    /// Centre on the display under the cursor, show, take the keyboard, and
    /// tell the window it was shown.
    fn show_focused(&self);
    fn hide(&self);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggled {
    Shown,
    Hidden,
}

/// The shortcut: shows a hidden window, hides a shown one.
pub fn toggle(w: &impl QuickSurface) -> Toggled {
    if w.is_visible() {
        w.hide();
        Toggled::Hidden
    } else {
        w.show_focused();
        Toggled::Shown
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HideReason {
    /// Esc, an action, the shortcut again, or the window's close button.
    Dismissed,
    /// The user clicked or switched elsewhere: focus already moved.
    Blurred,
    /// The vault locked.
    Locked,
}

/// macOS keeps HavenKeys the active app after its last window hides, so
/// the keyboard would not go back to the app the user came from. Hiding
/// the app fixes that, but also hides the main window: only when the user
/// dismissed the window and did not come from the main window.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn returns_focus_to_previous_app(main_was_focused: bool, reason: HideReason) -> bool {
    reason == HideReason::Dismissed && !main_was_focused
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutTarget {
    /// No vault on this computer yet, or one this build cannot read: the
    /// main window says what to do.
    Main,
    Quick,
}

/// `vault_exists`: `None` when the vault's status could not be read.
pub fn shortcut_target(vault_exists: Option<bool>) -> ShortcutTarget {
    match vault_exists {
        Some(true) => ShortcutTarget::Quick,
        _ => ShortcutTarget::Main,
    }
}

/// Top-left corner that centres a window on a monitor, in physical pixels.
/// A window larger than the monitor starts at the monitor's corner.
pub fn centered(
    monitor_pos: (i32, i32),
    monitor_size: (u32, u32),
    window_size: (u32, u32),
) -> (i32, i32) {
    let offset =
        |monitor: u32, window: u32| i32::try_from(monitor.saturating_sub(window) / 2).unwrap_or(0);
    (
        monitor_pos
            .0
            .saturating_add(offset(monitor_size.0, window_size.0)),
        monitor_pos
            .1
            .saturating_add(offset(monitor_size.1, window_size.1)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[derive(Default)]
    struct FakeHost {
        taken: Cell<bool>,
        registered: Cell<bool>,
        registers: Cell<u32>,
        unregisters: Cell<u32>,
    }

    impl ShortcutHost for FakeHost {
        fn register(&self) -> Result<(), ShortcutTaken> {
            self.registers.set(self.registers.get() + 1);
            if self.taken.get() {
                return Err(ShortcutTaken);
            }
            self.registered.set(true);
            Ok(())
        }
        fn unregister(&self) {
            self.unregisters.set(self.unregisters.get() + 1);
            self.registered.set(false);
        }
    }

    #[derive(Default)]
    struct FakeSurface {
        visible: Cell<bool>,
        shows: Cell<u32>,
        hides: Cell<u32>,
    }

    impl QuickSurface for FakeSurface {
        fn is_visible(&self) -> bool {
            self.visible.get()
        }
        fn show_focused(&self) {
            self.shows.set(self.shows.get() + 1);
            self.visible.set(true);
        }
        fn hide(&self) {
            self.hides.set(self.hides.get() + 1);
            self.visible.set(false);
        }
    }

    const ON: QuickAccessStatus = QuickAccessStatus {
        enabled: true,
        in_use: false,
    };
    const OFF: QuickAccessStatus = QuickAccessStatus {
        enabled: false,
        in_use: false,
    };

    #[test]
    fn the_setting_is_on_when_the_file_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            QuickAccessSetting::load(dir.path()),
            QuickAccessSetting { shortcut: true }
        );
        assert_eq!(QuickAccess::new(dir.path().into()).status(), ON);
    }

    #[test]
    fn the_setting_round_trips_without_leaving_a_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        QuickAccessSetting { shortcut: false }
            .save(dir.path())
            .unwrap();
        assert_eq!(
            QuickAccessSetting::load(dir.path()),
            QuickAccessSetting { shortcut: false }
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join(FILE)).unwrap(),
            r#"{"shortcut":false}"#
        );
        assert!(!dir.path().join(TMP).exists());
    }

    #[test]
    fn a_damaged_setting_falls_back_to_on() {
        let dir = tempfile::tempdir().unwrap();
        for bytes in [&b""[..], b"{", b"null", b"[]", b"{\"shortcut\":\"no\"}"] {
            std::fs::write(dir.path().join(FILE), bytes).unwrap();
            assert_eq!(
                QuickAccessSetting::load(dir.path()),
                QuickAccessSetting::default()
            );
        }
    }

    #[test]
    fn fields_from_a_newer_version_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE),
            br#"{"shortcut":false,"keys":"Alt+Space"}"#,
        )
        .unwrap();
        assert_eq!(
            QuickAccessSetting::load(dir.path()),
            QuickAccessSetting { shortcut: false }
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_setting_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        QuickAccessSetting { shortcut: true }
            .save(dir.path())
            .unwrap();
        let mode = std::fs::metadata(dir.path().join(FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_set_during_register_is_not_undone() {
        use std::sync::mpsc;
        use std::sync::{Arc, Mutex};
        struct Slow {
            started: Mutex<mpsc::Sender<()>>,
            go: Mutex<mpsc::Receiver<()>>,
            unregisters: Mutex<u32>,
        }
        impl ShortcutHost for Slow {
            fn register(&self) -> Result<(), ShortcutTaken> {
                self.started.lock().unwrap().send(()).unwrap();
                self.go.lock().unwrap().recv().unwrap();
                Ok(())
            }
            fn unregister(&self) {
                *self.unregisters.lock().unwrap() += 1;
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let qa = Arc::new(QuickAccess::new(dir.path().into()));
        let (started_tx, started_rx) = mpsc::channel();
        let (go_tx, go_rx) = mpsc::channel();
        let host = Arc::new(Slow {
            started: Mutex::new(started_tx),
            go: Mutex::new(go_rx),
            unregisters: Mutex::new(0),
        });
        let applier = {
            let (qa, host) = (qa.clone(), host.clone());
            std::thread::spawn(move || qa.apply(&*host))
        };
        started_rx.recv().unwrap();
        let setter = {
            let (qa, host) = (qa.clone(), host.clone());
            std::thread::spawn(move || qa.set(&*host, false).unwrap())
        };
        std::thread::sleep(std::time::Duration::from_millis(50));
        go_tx.send(()).unwrap();
        applier.join().unwrap();
        assert_eq!(setter.join().unwrap(), OFF);
        assert_eq!(qa.status(), OFF);
        assert_eq!(*host.unregisters.lock().unwrap(), 1);
        assert!(!QuickAccessSetting::load(dir.path()).shortcut);
    }

    #[test]
    fn a_stale_temp_file_does_not_loosen_the_mode() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(TMP), b"old").unwrap();
        QuickAccessSetting { shortcut: true }
            .save(dir.path())
            .unwrap();
        assert!(!dir.path().join(TMP).exists());
        assert_eq!(
            QuickAccessSetting::load(dir.path()),
            QuickAccessSetting { shortcut: true }
        );
    }

    #[test]
    fn applying_registers_once() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        assert_eq!(qa.apply(&host), ON);
        assert_eq!(qa.apply(&host), ON);
        assert_eq!(host.registers.get(), 1);
        assert!(host.registered.get());
    }

    #[test]
    fn turning_it_off_unregisters_and_is_remembered() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        qa.apply(&host);
        assert_eq!(qa.set(&host, false).unwrap(), OFF);
        assert_eq!(host.unregisters.get(), 1);
        assert!(!host.registered.get());
        // The next start reads the file and never registers.
        let next = QuickAccess::new(dir.path().into());
        let fresh = FakeHost::default();
        assert_eq!(next.apply(&fresh), OFF);
        assert_eq!(fresh.registers.get(), 0);
    }

    #[test]
    fn turning_it_on_registers_at_once() {
        let dir = tempfile::tempdir().unwrap();
        QuickAccessSetting { shortcut: false }
            .save(dir.path())
            .unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        assert_eq!(qa.apply(&host), OFF);
        assert_eq!(qa.set(&host, true).unwrap(), ON);
        assert_eq!(host.registers.get(), 1);
        assert_eq!(
            QuickAccessSetting::load(dir.path()),
            QuickAccessSetting { shortcut: true }
        );
    }

    #[test]
    fn a_taken_shortcut_is_reported_and_the_switch_stays_on() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        host.taken.set(true);
        assert_eq!(
            qa.apply(&host),
            QuickAccessStatus {
                enabled: true,
                in_use: true
            }
        );
        // Off: nothing was registered, so nothing to unregister.
        assert_eq!(qa.set(&host, false).unwrap(), OFF);
        assert_eq!(host.unregisters.get(), 0);
        // The other app let go; on again retries.
        host.taken.set(false);
        assert_eq!(qa.set(&host, true).unwrap(), ON);
        assert!(host.registered.get());
    }

    #[test]
    fn a_setting_that_cannot_be_saved_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().join("gone"));
        let host = FakeHost::default();
        qa.apply(&host);
        assert!(qa.set(&host, false).is_err());
        assert_eq!(qa.status(), ON);
        assert_eq!(host.unregisters.get(), 0);
    }

    #[test]
    fn the_shortcut_shows_a_hidden_window_and_hides_a_shown_one() {
        let w = FakeSurface::default();
        assert_eq!(toggle(&w), Toggled::Shown);
        assert_eq!((w.shows.get(), w.hides.get()), (1, 0));
        assert_eq!(toggle(&w), Toggled::Hidden);
        assert_eq!((w.shows.get(), w.hides.get()), (1, 1));
    }

    #[test]
    fn the_shortcut_opens_the_main_window_without_a_vault() {
        assert_eq!(shortcut_target(Some(true)), ShortcutTarget::Quick);
        assert_eq!(shortcut_target(Some(false)), ShortcutTarget::Main);
        assert_eq!(shortcut_target(None), ShortcutTarget::Main);
    }

    #[test]
    fn only_a_dismissal_from_another_app_hides_the_app() {
        assert!(returns_focus_to_previous_app(false, HideReason::Dismissed));
        assert!(!returns_focus_to_previous_app(true, HideReason::Dismissed));
        for main in [false, true] {
            assert!(!returns_focus_to_previous_app(main, HideReason::Blurred));
            assert!(!returns_focus_to_previous_app(main, HideReason::Locked));
        }
    }

    #[test]
    fn the_window_is_centred_on_the_display() {
        assert_eq!(centered((0, 0), (1920, 1080), (640, 420)), (640, 330));
        // A second monitor to the right, and one to the left (negative x).
        assert_eq!(
            centered((1920, 0), (2560, 1440), (640, 420)),
            (1920 + 960, 510)
        );
        assert_eq!(centered((-1920, 0), (1920, 1080), (640, 420)), (-1280, 330));
        // Larger than the display: its corner.
        assert_eq!(centered((100, 50), (600, 400), (640, 420)), (100, 50));
    }
}
