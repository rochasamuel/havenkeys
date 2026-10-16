//! Quick Access: the global shortcut and the small search window (spec
//! docs/superpowers/specs/2026-10-13-quick-access-design.md).
//!
//! The decisions live here, behind two small traits so the tests need no
//! OS hook and no window: `ShortcutHost` registers the chosen shortcut
//! (default Cmd/Ctrl+Shift+Space; spec
//! 2026-10-14-quick-access-shortcut-remap-design) in quick_shortcut.rs,
//! `QuickSurface` shows and hides the window (quick_window.rs).

use serde::{Deserialize, Deserializer, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

pub const QUICK_WINDOW: &str = "quick";
/// Sent to the quick window each time it is shown: it re-reads the lock
/// state, starts empty and focuses its field. Carries [`Shown`].
pub const SHOWN_EVENT: &str = "quick://shown";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shown {
    /// Opened by the browser's Unlock: it only unlocks, then hides.
    pub for_browser: bool,
}
const FILE: &str = "quick-access.json";
const TMP: &str = "quick-access.json.tmp";

/// The longest chord string taken from the renderer or the file.
const MAX_KEYS_LEN: usize = 64;

/// The keys a chord may end with: the `Code` variant (named as the DOM's
/// `KeyboardEvent.code`) and its name in the chord string. The renderer maps
/// the same codes in src/lib/shortcut.ts; shortcut.test.ts reads this table
/// and fails if the two differ. Keep one `(Code::X, "name")` per entry.
const ALLOWED_KEYS: &[(Code, &str)] = &[
    (Code::KeyA, "A"),
    (Code::KeyB, "B"),
    (Code::KeyC, "C"),
    (Code::KeyD, "D"),
    (Code::KeyE, "E"),
    (Code::KeyF, "F"),
    (Code::KeyG, "G"),
    (Code::KeyH, "H"),
    (Code::KeyI, "I"),
    (Code::KeyJ, "J"),
    (Code::KeyK, "K"),
    (Code::KeyL, "L"),
    (Code::KeyM, "M"),
    (Code::KeyN, "N"),
    (Code::KeyO, "O"),
    (Code::KeyP, "P"),
    (Code::KeyQ, "Q"),
    (Code::KeyR, "R"),
    (Code::KeyS, "S"),
    (Code::KeyT, "T"),
    (Code::KeyU, "U"),
    (Code::KeyV, "V"),
    (Code::KeyW, "W"),
    (Code::KeyX, "X"),
    (Code::KeyY, "Y"),
    (Code::KeyZ, "Z"),
    (Code::Digit0, "0"),
    (Code::Digit1, "1"),
    (Code::Digit2, "2"),
    (Code::Digit3, "3"),
    (Code::Digit4, "4"),
    (Code::Digit5, "5"),
    (Code::Digit6, "6"),
    (Code::Digit7, "7"),
    (Code::Digit8, "8"),
    (Code::Digit9, "9"),
    (Code::Space, "Space"),
    (Code::F1, "F1"),
    (Code::F2, "F2"),
    (Code::F3, "F3"),
    (Code::F4, "F4"),
    (Code::F5, "F5"),
    (Code::F6, "F6"),
    (Code::F7, "F7"),
    (Code::F8, "F8"),
    (Code::F9, "F9"),
    (Code::F10, "F10"),
    (Code::F11, "F11"),
    (Code::F12, "F12"),
    (Code::Backquote, "Backquote"),
    (Code::Minus, "Minus"),
    (Code::Equal, "Equal"),
    (Code::BracketLeft, "BracketLeft"),
    (Code::BracketRight, "BracketRight"),
    (Code::Backslash, "Backslash"),
    (Code::Semicolon, "Semicolon"),
    (Code::Quote, "Quote"),
    (Code::Comma, "Comma"),
    (Code::Period, "Period"),
    (Code::Slash, "Slash"),
];

/// System and editing chords, matched exactly (adding Shift makes another
/// chord). Cmd/Ctrl+Tab, Alt+Tab and Ctrl+Alt+Delete never get this far:
/// Tab and Delete are not allowed keys.
const RESERVED: &[&str] = &[
    "Super+C",
    "Control+C",
    "Super+V",
    "Control+V",
    "Super+X",
    "Control+X",
    "Super+A",
    "Control+A",
    "Super+Z",
    "Control+Z",
    "Super+Y",
    "Control+Y",
    "Super+Q",
    "Control+Q",
    "Super+W",
    "Control+W",
    "Super+Space",
    "Super+Shift+3",
    "Super+Shift+4",
    "Super+Shift+5",
    "Alt+F4",
    "Super+Control+Q",
];

/// Modifiers in the order the chord string writes them.
const MODIFIER_NAMES: [(Modifiers, &str); 4] = [
    (Modifiers::SUPER, "Super"),
    (Modifiers::CONTROL, "Control"),
    (Modifiers::ALT, "Alt"),
    (Modifiers::SHIFT, "Shift"),
];

/// `Cmd+Shift+Space` on macOS, `Ctrl+Shift+Space` elsewhere.
pub fn default_shortcut() -> Shortcut {
    let primary = if cfg!(target_os = "macos") {
        Modifiers::SUPER
    } else {
        Modifiers::CONTROL
    };
    Shortcut::new(Some(primary | Modifiers::SHIFT), Code::Space)
}

fn canonical(s: &Shortcut) -> Option<String> {
    let key = ALLOWED_KEYS
        .iter()
        .find(|(code, _)| *code == s.key)
        .map(|(_, name)| *name)?;
    let mut out = String::new();
    for (m, name) in MODIFIER_NAMES {
        if s.mods.contains(m) {
            out.push_str(name);
            out.push('+');
        }
    }
    out.push_str(key);
    Some(out)
}

/// The chord string for a shortcut that passed `parse_keys` (or the
/// default). Empty for a key outside `ALLOWED_KEYS`, which never happens.
pub fn keys_of(s: &Shortcut) -> String {
    canonical(s).unwrap_or_default()
}

/// Why a chord was refused. Each maps to one fixed message
/// (quick_shortcut.rs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutRefusal {
    /// No Cmd (Super), Ctrl or Alt.
    Modifier,
    /// Not an allowed key, or not the exact chord spelling.
    Key,
    /// A system or editing chord.
    Reserved,
}

/// The rules of spec §4, for the renderer's proposal and the saved file
/// alike. Only the exact spelling `keys_of` produces is accepted, so the
/// plugin's aliases (Ctrl, Cmd, CmdOrCtrl), other orders, case and spaces
/// are refused.
pub fn parse_keys(keys: &str) -> Result<Shortcut, ShortcutRefusal> {
    if keys.len() > MAX_KEYS_LEN {
        return Err(ShortcutRefusal::Key);
    }
    let parsed: Shortcut = keys.parse().map_err(|_| ShortcutRefusal::Key)?;
    if canonical(&parsed).as_deref() != Some(keys) {
        return Err(ShortcutRefusal::Key);
    }
    if !parsed
        .mods
        .intersects(Modifiers::SUPER | Modifiers::CONTROL | Modifiers::ALT)
    {
        return Err(ShortcutRefusal::Modifier);
    }
    if RESERVED.contains(&keys) {
        return Err(ShortcutRefusal::Reserved);
    }
    Ok(parsed)
}

/// What the file stores for a chord: nothing for the default.
fn stored_keys(s: &Shortcut) -> Option<String> {
    (*s != default_shortcut()).then(|| keys_of(s))
}

/// A string, or nothing for any other JSON value: a bad chord must not make
/// the whole file unreadable (which would turn the switch back on).
fn string_or_none<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::String(s) => Some(s),
        _ => None,
    })
}

/// This computer's choice, outside the vault: the shortcut must work while
/// the vault is locked (to unlock from Quick Access), and the vault's
/// settings are sealed under the vault key. Holds nothing secret
/// (security-model.md §4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickAccessSetting {
    #[serde(default = "default_on")]
    pub shortcut: bool,
    /// The chosen chord; absent for the default. Checked by `parse_keys` on
    /// load like any proposal.
    #[serde(
        default,
        deserialize_with = "string_or_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub shortcut_keys: Option<String>,
}

fn default_on() -> bool {
    true
}

impl Default for QuickAccessSetting {
    fn default() -> Self {
        Self {
            shortcut: true,
            shortcut_keys: None,
        }
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

    /// The saved chord if it passes the rules, else the default.
    pub fn chord(&self) -> Shortcut {
        self.shortcut_keys
            .as_deref()
            .and_then(|k| parse_keys(k).ok())
            .unwrap_or_else(default_shortcut)
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

/// Registers a shortcut with the OS. Implemented over the plugin in
/// quick_shortcut.rs; faked in the tests.
pub trait ShortcutHost {
    fn register(&self, shortcut: &Shortcut) -> Result<(), ShortcutTaken>;
    fn unregister(&self, shortcut: &Shortcut);
}

/// What Settings shows: the switch, whether the OS refused the current
/// chord, the chord itself, and (in `set_shortcut`'s answer only) whether
/// the proposed chord was taken.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickAccessStatus {
    pub enabled: bool,
    pub in_use: bool,
    /// The current chord, in `parse_keys` form.
    pub keys: String,
    pub is_default: bool,
    /// The proposed chord is held by another app; nothing changed.
    pub candidate_in_use: bool,
}

#[derive(Debug)]
pub enum SetShortcutError {
    Refused(ShortcutRefusal),
    /// quick-access.json could not be written; nothing changed. The cause
    /// is kept for `Debug` only: the command answers one fixed message.
    Save(#[allow(dead_code)] std::io::Error),
}

#[derive(Clone, Copy)]
struct Registration {
    enabled: bool,
    /// The chord Quick Access answers to (registered or not).
    shortcut: Shortcut,
    registered: bool,
    in_use: bool,
}

pub struct QuickAccess {
    dir: PathBuf,
    registration: Mutex<Registration>,
    /// Serializes `apply`, `set` and `set_shortcut`. Held across the host's
    /// register/unregister calls, which wait for the main thread;
    /// `registration` never is, so `status()` (a sync command, on the main
    /// thread) and the shortcut handler cannot deadlock against it.
    applying: Mutex<()>,
    /// Whether the main window had the keyboard when the quick window
    /// opened (macOS: hiding the app returns focus to the previous app).
    main_was_focused: AtomicBool,
    /// Showing for the browser's Unlock: the next unlock hides it.
    for_browser: AtomicBool,
}

impl QuickAccess {
    pub fn new(dir: PathBuf) -> Self {
        let setting = QuickAccessSetting::load(&dir);
        Self {
            registration: Mutex::new(Registration {
                enabled: setting.shortcut,
                shortcut: setting.chord(),
                registered: false,
                in_use: false,
            }),
            dir,
            applying: Mutex::new(()),
            main_was_focused: AtomicBool::new(false),
            for_browser: AtomicBool::new(false),
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
            keys: keys_of(&r.shortcut),
            is_default: r.shortcut == default_shortcut(),
            candidate_in_use: false,
        }
    }

    /// The plugin's handler: is this press the chord Quick Access answers to?
    pub fn is_current(&self, pressed: &Shortcut) -> bool {
        let current = self.snapshot().shortcut;
        current.mods == pressed.mods && current.key == pressed.key
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
            match host.register(&r.shortcut) {
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
            host.unregister(&r.shortcut);
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
        // Only what this call changed; `enabled` and `shortcut` are never
        // written back here.
        {
            let mut current = self.registration.lock().unwrap_or_else(|p| p.into_inner());
            current.registered = next.registered;
            current.in_use = next.in_use;
        }
        self.status()
    }

    /// The Settings switch: saved first (keeping the chosen chord), so if
    /// the file cannot be written nothing changes; then registered or
    /// unregistered at once.
    pub fn set(
        &self,
        host: &impl ShortcutHost,
        enabled: bool,
    ) -> std::io::Result<QuickAccessStatus> {
        // Held across save, enable and apply, so two sets cannot interleave.
        let _one_at_a_time = self.applying.lock().unwrap_or_else(|p| p.into_inner());
        let shortcut = self.snapshot().shortcut;
        QuickAccessSetting {
            shortcut: enabled,
            shortcut_keys: stored_keys(&shortcut),
        }
        .save(&self.dir)?;
        self.registration
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .enabled = enabled;
        Ok(self.apply_locked(host))
    }

    /// Settings → Change… / Reset (spec §5). `None` is the default chord.
    /// Refused by the rules → error, nothing touched. Switch on: the new
    /// chord is registered while the old one still is; taken → the status
    /// with `candidate_in_use`, nothing changed; then the file is saved
    /// (failure → the new chord is released, the old one never was) and
    /// only then the old chord is released. Switch off: only saved. The
    /// same chord is a no-op, except that one another app held is tried
    /// again (Reset, or choosing it again, once that app let go).
    pub fn set_shortcut(
        &self,
        host: &impl ShortcutHost,
        keys: Option<&str>,
    ) -> Result<QuickAccessStatus, SetShortcutError> {
        let _one_at_a_time = self.applying.lock().unwrap_or_else(|p| p.into_inner());
        let next = match keys {
            None => default_shortcut(),
            Some(k) => parse_keys(k).map_err(SetShortcutError::Refused)?,
        };
        let r = self.snapshot();
        if next == r.shortcut {
            return Ok(if r.enabled && !r.registered {
                self.apply_locked(host)
            } else {
                self.status()
            });
        }
        let setting = QuickAccessSetting {
            shortcut: r.enabled,
            shortcut_keys: stored_keys(&next),
        };
        if r.enabled {
            if host.register(&next).is_err() {
                return Ok(QuickAccessStatus {
                    candidate_in_use: true,
                    ..self.status()
                });
            }
            if let Err(e) = setting.save(&self.dir) {
                host.unregister(&next);
                return Err(SetShortcutError::Save(e));
            }
            if r.registered {
                host.unregister(&r.shortcut);
            }
        } else {
            setting.save(&self.dir).map_err(SetShortcutError::Save)?;
        }
        {
            let mut current = self.registration.lock().unwrap_or_else(|p| p.into_inner());
            current.shortcut = next;
            current.registered = r.enabled;
            current.in_use = false;
        }
        Ok(self.status())
    }

    pub fn note_opened(&self, main_was_focused: bool, shown: Shown) {
        self.main_was_focused
            .store(main_was_focused, Ordering::Relaxed);
        self.for_browser.store(shown.for_browser, Ordering::Relaxed);
    }

    pub fn note_hidden(&self) {
        self.for_browser.store(false, Ordering::Relaxed);
    }

    /// True once after an opening for the browser, until the window hides.
    pub fn take_browser_unlock(&self) -> bool {
        self.for_browser.swap(false, Ordering::Relaxed)
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
    fn show_focused(&self, shown: Shown);
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
        w.show_focused(Shown { for_browser: false });
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

/// Where the browser's Unlock (`show_unlock`) goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserUnlockTarget {
    /// No vault, or its status is unreadable.
    Main,
    Quick,
    /// Already unlocked.
    Nothing,
}

/// `status`: `(vault_exists, unlocked)`.
pub fn browser_unlock_target(status: Option<(bool, bool)>) -> BrowserUnlockTarget {
    match status {
        Some((true, true)) => BrowserUnlockTarget::Nothing,
        Some((true, false)) => BrowserUnlockTarget::Quick,
        _ => BrowserUnlockTarget::Main,
    }
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
    use std::cell::{Cell, RefCell};

    #[test]
    fn every_allowed_key_parses_unless_reserved() {
        for (_, name) in ALLOWED_KEYS {
            let chord = format!("Control+Shift+{name}");
            if RESERVED.contains(&chord.as_str()) {
                continue;
            }
            assert!(parse_keys(&chord).is_ok(), "{chord} should parse");
        }
    }

    /// A fake OS. `log` records "+keys" for every register attempt and
    /// "-keys" for every unregister, in order.
    #[derive(Default)]
    struct FakeHost {
        /// Every chord is refused (another app holds them all, or no plugin).
        taken_all: Cell<bool>,
        /// Chords another app holds.
        taken: RefCell<Vec<Shortcut>>,
        held: RefCell<Vec<Shortcut>>,
        log: RefCell<Vec<String>>,
    }

    impl FakeHost {
        fn take(&self, keys: &str) {
            self.taken.borrow_mut().push(chord(keys));
        }
        fn holds(&self, keys: &str) -> bool {
            self.held.borrow().contains(&chord(keys))
        }
        fn registers(&self) -> usize {
            self.log
                .borrow()
                .iter()
                .filter(|l| l.starts_with('+'))
                .count()
        }
        fn unregisters(&self) -> usize {
            self.log
                .borrow()
                .iter()
                .filter(|l| l.starts_with('-'))
                .count()
        }
        fn log(&self) -> Vec<String> {
            self.log.borrow().clone()
        }
    }

    impl ShortcutHost for FakeHost {
        fn register(&self, shortcut: &Shortcut) -> Result<(), ShortcutTaken> {
            self.log
                .borrow_mut()
                .push(format!("+{}", keys_of(shortcut)));
            if self.taken_all.get() || self.taken.borrow().contains(shortcut) {
                return Err(ShortcutTaken);
            }
            self.held.borrow_mut().push(*shortcut);
            Ok(())
        }
        fn unregister(&self, shortcut: &Shortcut) {
            self.log
                .borrow_mut()
                .push(format!("-{}", keys_of(shortcut)));
            self.held.borrow_mut().retain(|h| h != shortcut);
        }
    }

    #[derive(Default)]
    struct FakeSurface {
        visible: Cell<bool>,
        shows: Cell<u32>,
        hides: Cell<u32>,
        last: Cell<Option<Shown>>,
    }

    impl QuickSurface for FakeSurface {
        fn is_visible(&self) -> bool {
            self.visible.get()
        }
        fn show_focused(&self, shown: Shown) {
            self.last.set(Some(shown));
            self.shows.set(self.shows.get() + 1);
            self.visible.set(true);
        }
        fn hide(&self) {
            self.hides.set(self.hides.get() + 1);
            self.visible.set(false);
        }
    }

    /// Any chord string the plugin can parse (the tests also need refused ones).
    fn chord(keys: &str) -> Shortcut {
        keys.parse().unwrap()
    }
    fn dflt() -> String {
        keys_of(&default_shortcut())
    }
    fn switch(on: bool) -> QuickAccessSetting {
        QuickAccessSetting {
            shortcut: on,
            shortcut_keys: None,
        }
    }
    fn status(enabled: bool, in_use: bool) -> QuickAccessStatus {
        QuickAccessStatus {
            enabled,
            in_use,
            keys: dflt(),
            is_default: true,
            candidate_in_use: false,
        }
    }
    fn on() -> QuickAccessStatus {
        status(true, false)
    }
    fn off() -> QuickAccessStatus {
        status(false, false)
    }
    const ALT: &str = "Alt+Shift+Space";

    // ------------------------------------------------------------ rules

    #[test]
    fn the_default_is_command_or_control_shift_space() {
        let (primary, other) = if cfg!(target_os = "macos") {
            (Modifiers::SUPER, Modifiers::CONTROL)
        } else {
            (Modifiers::CONTROL, Modifiers::SUPER)
        };
        let d = default_shortcut();
        assert_eq!(d.mods, primary | Modifiers::SHIFT);
        assert!(!d.mods.intersects(other | Modifiers::ALT));
        assert_eq!(d.key, Code::Space);
        let want = if cfg!(target_os = "macos") {
            "Super+Shift+Space"
        } else {
            "Control+Shift+Space"
        };
        assert_eq!(dflt(), want);
        // The default passes its own rules.
        assert_eq!(parse_keys(&dflt()), Ok(d));
    }

    #[test]
    fn chords_that_are_accepted() {
        for keys in [
            "Alt+Shift+Space",
            "Super+Shift+K",
            "Control+Shift+Space",
            "Super+Shift+Space",
            "Control+Alt+0",
            "Super+Control+Alt+Shift+F12",
            "Control+Backquote",
            "Alt+Slash",
            "Control+Shift+BracketLeft",
            "Super+Shift+6",
            "Control+Shift+C",
            "Control+L",
            "Alt+F1",
        ] {
            let s = parse_keys(keys).unwrap_or_else(|r| panic!("{keys}: {r:?}"));
            assert_eq!(keys_of(&s), keys);
        }
    }

    #[test]
    fn a_chord_needs_cmd_ctrl_or_alt() {
        for keys in ["K", "Shift+K", "Space", "Shift+F1", "Shift+Space"] {
            assert_eq!(parse_keys(keys), Err(ShortcutRefusal::Modifier), "{keys}");
        }
    }

    #[test]
    fn keys_outside_the_list_and_other_spellings_are_refused() {
        let long = format!("Control+{}", "A".repeat(60));
        for keys in [
            "",
            "+",
            "++",
            "Control+",
            "Control+Shift",
            "Control++",
            "Control+K+J",
            "Control+Tab",
            "Alt+Tab",
            "Shift+Tab",
            "Control+Alt+Delete",
            "Control+Shift+Enter",
            "Control+Escape",
            "Control+ArrowUp",
            "Control+F13",
            "Control+Numpad1",
            "Control+Backspace",
            // Aliases, order, case and spacing the plugin would take.
            "Ctrl+Shift+Space",
            "Shift+Control+Space",
            "control+shift+space",
            "Control + K",
            "Control+KeyK",
            "Control+Digit1",
            "CmdOrCtrl+K",
            "Command+K",
            "Control+-",
            "Control+K\0",
            "Control+\u{212a}",
            long.as_str(),
        ] {
            assert_eq!(parse_keys(keys), Err(ShortcutRefusal::Key), "{keys:?}");
        }
    }

    #[test]
    fn reserved_chords_are_refused() {
        for keys in [
            "Super+C",
            "Control+C",
            "Super+V",
            "Control+V",
            "Super+X",
            "Control+X",
            "Super+A",
            "Control+A",
            "Super+Z",
            "Control+Z",
            "Super+Y",
            "Control+Y",
            "Super+Q",
            "Control+Q",
            "Super+W",
            "Control+W",
            "Super+Space",
            "Super+Shift+3",
            "Super+Shift+4",
            "Super+Shift+5",
            "Alt+F4",
            "Super+Control+Q",
        ] {
            assert_eq!(parse_keys(keys), Err(ShortcutRefusal::Reserved), "{keys}");
        }
    }

    // ---------------------------------------------------------- setting

    #[test]
    fn the_setting_is_on_when_the_file_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(QuickAccessSetting::load(dir.path()), switch(true));
        assert_eq!(QuickAccess::new(dir.path().into()).status(), on());
    }

    #[test]
    fn the_setting_round_trips_without_leaving_a_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        switch(false).save(dir.path()).unwrap();
        assert_eq!(QuickAccessSetting::load(dir.path()), switch(false));
        // The default chord is not written.
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
        assert_eq!(QuickAccessSetting::load(dir.path()), switch(false));
    }

    #[test]
    fn a_saved_chord_is_used_at_start() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE),
            br#"{"shortcut":true,"shortcut_keys":"Alt+Shift+Space"}"#,
        )
        .unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let s = qa.status();
        assert_eq!((s.keys.as_str(), s.is_default), (ALT, false));
        let host = FakeHost::default();
        qa.apply(&host);
        assert_eq!(host.log(), [format!("+{ALT}")]);
    }

    #[test]
    fn a_bad_saved_chord_falls_back_to_the_default() {
        let dir = tempfile::tempdir().unwrap();
        for value in [
            r#""Control+C""#,
            r#""garbage""#,
            r#""Ctrl+Shift+K""#,
            r#""K""#,
            "5",
            "null",
            "{}",
            "[]",
        ] {
            let text = format!(r#"{{"shortcut":false,"shortcut_keys":{value}}}"#);
            std::fs::write(dir.path().join(FILE), text).unwrap();
            let setting = QuickAccessSetting::load(dir.path());
            // The switch survives a bad chord.
            assert!(!setting.shortcut, "{value}");
            assert_eq!(setting.chord(), default_shortcut(), "{value}");
            assert_eq!(
                QuickAccess::new(dir.path().into()).status(),
                off(),
                "{value}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_setting_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        switch(true).save(dir.path()).unwrap();
        let mode = std::fs::metadata(dir.path().join(FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_stale_temp_file_does_not_loosen_the_mode() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(TMP), b"old").unwrap();
        switch(true).save(dir.path()).unwrap();
        assert!(!dir.path().join(TMP).exists());
        assert_eq!(QuickAccessSetting::load(dir.path()), switch(true));
    }

    // ------------------------------------------------------ the switch

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
            fn register(&self, _: &Shortcut) -> Result<(), ShortcutTaken> {
                self.started.lock().unwrap().send(()).unwrap();
                self.go.lock().unwrap().recv().unwrap();
                Ok(())
            }
            fn unregister(&self, _: &Shortcut) {
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
        assert_eq!(setter.join().unwrap(), off());
        assert_eq!(qa.status(), off());
        assert_eq!(*host.unregisters.lock().unwrap(), 1);
        assert!(!QuickAccessSetting::load(dir.path()).shortcut);
    }

    #[test]
    fn applying_registers_once() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        assert_eq!(qa.apply(&host), on());
        assert_eq!(qa.apply(&host), on());
        assert_eq!(host.registers(), 1);
        assert!(host.holds(&dflt()));
    }

    #[test]
    fn turning_it_off_unregisters_and_is_remembered() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        qa.apply(&host);
        assert_eq!(qa.set(&host, false).unwrap(), off());
        assert_eq!(host.unregisters(), 1);
        assert!(!host.holds(&dflt()));
        // The next start reads the file and never registers.
        let next = QuickAccess::new(dir.path().into());
        let fresh = FakeHost::default();
        assert_eq!(next.apply(&fresh), off());
        assert_eq!(fresh.registers(), 0);
    }

    #[test]
    fn turning_it_on_registers_at_once() {
        let dir = tempfile::tempdir().unwrap();
        switch(false).save(dir.path()).unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        assert_eq!(qa.apply(&host), off());
        assert_eq!(qa.set(&host, true).unwrap(), on());
        assert_eq!(host.registers(), 1);
        assert_eq!(QuickAccessSetting::load(dir.path()), switch(true));
    }

    #[test]
    fn a_taken_shortcut_is_reported_and_the_switch_stays_on() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        host.taken_all.set(true);
        assert_eq!(qa.apply(&host), status(true, true));
        // Off: nothing was registered, so nothing to unregister.
        assert_eq!(qa.set(&host, false).unwrap(), off());
        assert_eq!(host.unregisters(), 0);
        // The other app let go; on again retries.
        host.taken_all.set(false);
        assert_eq!(qa.set(&host, true).unwrap(), on());
        assert!(host.holds(&dflt()));
    }

    #[test]
    fn a_setting_that_cannot_be_saved_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().join("gone"));
        let host = FakeHost::default();
        qa.apply(&host);
        assert!(qa.set(&host, false).is_err());
        assert_eq!(qa.status(), on());
        assert_eq!(host.unregisters(), 0);
    }

    #[test]
    fn the_switch_keeps_a_custom_chord() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        qa.apply(&host);
        qa.set_shortcut(&host, Some(ALT)).unwrap();
        qa.set(&host, false).unwrap();
        assert_eq!(
            QuickAccessSetting::load(dir.path())
                .shortcut_keys
                .as_deref(),
            Some(ALT)
        );
        assert_eq!(host.log().last().unwrap(), &format!("-{ALT}"));
        qa.set(&host, true).unwrap();
        assert_eq!(host.log().last().unwrap(), &format!("+{ALT}"));
        // And after a restart.
        assert_eq!(QuickAccess::new(dir.path().into()).status().keys, ALT);
    }

    // --------------------------------------------------- changing chord

    #[test]
    fn switching_registers_the_new_chord_before_releasing_the_old() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        qa.apply(&host);
        let s = qa.set_shortcut(&host, Some(ALT)).unwrap();
        assert_eq!(
            s,
            QuickAccessStatus {
                enabled: true,
                in_use: false,
                keys: ALT.into(),
                is_default: false,
                candidate_in_use: false,
            }
        );
        assert_eq!(
            host.log(),
            [
                format!("+{}", dflt()),
                format!("+{ALT}"),
                format!("-{}", dflt())
            ]
        );
        assert!(host.holds(ALT) && !host.holds(&dflt()));
        assert_eq!(
            std::fs::read_to_string(dir.path().join(FILE)).unwrap(),
            r#"{"shortcut":true,"shortcut_keys":"Alt+Shift+Space"}"#
        );
    }

    #[test]
    fn a_taken_chord_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        qa.apply(&host);
        host.take(ALT);
        let s = qa.set_shortcut(&host, Some(ALT)).unwrap();
        assert!(s.candidate_in_use);
        assert_eq!((s.keys.clone(), s.in_use), (dflt(), false));
        assert!(host.holds(&dflt()));
        assert_eq!(host.unregisters(), 0);
        assert!(!dir.path().join(FILE).exists());
        // The flag belongs to that one answer.
        assert!(!qa.status().candidate_in_use);
    }

    #[test]
    fn a_refused_chord_changes_nothing_and_never_reaches_the_os() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        qa.apply(&host);
        for (keys, why) in [
            ("Control+C", ShortcutRefusal::Reserved),
            ("Shift+K", ShortcutRefusal::Modifier),
            ("Control+Tab", ShortcutRefusal::Key),
        ] {
            match qa.set_shortcut(&host, Some(keys)) {
                Err(SetShortcutError::Refused(r)) => assert_eq!(r, why, "{keys}"),
                other => panic!("{keys}: {other:?}"),
            }
        }
        assert_eq!(host.registers(), 1);
        assert_eq!(qa.status(), on());
        assert!(!dir.path().join(FILE).exists());
    }

    #[test]
    fn the_same_chord_is_a_no_op() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        qa.apply(&host);
        assert_eq!(qa.set_shortcut(&host, Some(dflt().as_str())).unwrap(), on());
        assert_eq!(qa.set_shortcut(&host, None).unwrap(), on());
        assert_eq!(host.registers(), 1);
        assert!(!dir.path().join(FILE).exists());
    }

    #[test]
    fn the_same_chord_retries_one_another_app_held() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        host.taken_all.set(true);
        assert_eq!(qa.apply(&host), status(true, true));
        // Still taken: still in use, and nothing saved.
        assert_eq!(qa.set_shortcut(&host, None).unwrap(), status(true, true));
        assert_eq!(host.registers(), 2);
        // The other app let go: Reset (or choosing it again) takes it.
        host.taken_all.set(false);
        assert_eq!(qa.set_shortcut(&host, None).unwrap(), on());
        assert!(host.holds(&dflt()));
        assert_eq!(qa.set_shortcut(&host, Some(dflt().as_str())).unwrap(), on());
        assert_eq!(host.registers(), 3);
        assert!(!dir.path().join(FILE).exists());
    }

    #[test]
    fn a_save_failure_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().join("gone"));
        let host = FakeHost::default();
        qa.apply(&host);
        assert!(matches!(
            qa.set_shortcut(&host, Some(ALT)),
            Err(SetShortcutError::Save(_))
        ));
        assert_eq!(
            host.log(),
            [format!("+{}", dflt()), format!("+{ALT}"), format!("-{ALT}")]
        );
        assert!(host.holds(&dflt()) && !host.holds(ALT));
        assert_eq!(qa.status(), on());
        assert!(qa.is_current(&default_shortcut()));
    }

    #[test]
    fn with_the_switch_off_a_change_is_only_saved() {
        let dir = tempfile::tempdir().unwrap();
        switch(false).save(dir.path()).unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        qa.apply(&host);
        let s = qa.set_shortcut(&host, Some(ALT)).unwrap();
        assert_eq!(
            (s.enabled, s.keys.as_str(), s.candidate_in_use),
            (false, ALT, false)
        );
        assert!(host.log().is_empty());
        assert_eq!(
            QuickAccessSetting::load(dir.path())
                .shortcut_keys
                .as_deref(),
            Some(ALT)
        );
        // Turning it on registers the chord chosen while off.
        qa.set(&host, true).unwrap();
        assert_eq!(host.log(), [format!("+{ALT}")]);
    }

    #[test]
    fn a_new_chord_replaces_one_another_app_held() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        host.take(&dflt());
        assert!(qa.apply(&host).in_use);
        let s = qa.set_shortcut(&host, Some(ALT)).unwrap();
        assert_eq!((s.in_use, s.keys.as_str()), (false, ALT));
        // The old chord was never ours: nothing to release.
        assert_eq!(host.unregisters(), 0);
    }

    #[test]
    fn reset_returns_to_the_default_and_clears_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        qa.apply(&host);
        qa.set_shortcut(&host, Some(ALT)).unwrap();
        assert_eq!(qa.set_shortcut(&host, None).unwrap(), on());
        assert!(host.holds(&dflt()) && !host.holds(ALT));
        assert_eq!(
            std::fs::read_to_string(dir.path().join(FILE)).unwrap(),
            r#"{"shortcut":true}"#
        );
    }

    #[test]
    fn the_handler_matches_only_the_current_chord() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        let host = FakeHost::default();
        qa.apply(&host);
        assert!(qa.is_current(&default_shortcut()));
        assert!(!qa.is_current(&chord(ALT)));
        qa.set_shortcut(&host, Some(ALT)).unwrap();
        assert!(qa.is_current(&chord(ALT)));
        assert!(!qa.is_current(&default_shortcut()));
    }

    #[test]
    fn a_chord_change_waits_for_a_running_apply() {
        use std::sync::mpsc;
        use std::sync::{Arc, Mutex};
        struct SlowOnce {
            started: Mutex<Option<mpsc::Sender<()>>>,
            go: Mutex<mpsc::Receiver<()>>,
            log: Mutex<Vec<String>>,
        }
        impl ShortcutHost for SlowOnce {
            fn register(&self, s: &Shortcut) -> Result<(), ShortcutTaken> {
                if let Some(started) = self.started.lock().unwrap().take() {
                    started.send(()).unwrap();
                    self.go.lock().unwrap().recv().unwrap();
                }
                self.log.lock().unwrap().push(format!("+{}", keys_of(s)));
                Ok(())
            }
            fn unregister(&self, s: &Shortcut) {
                self.log.lock().unwrap().push(format!("-{}", keys_of(s)));
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let qa = Arc::new(QuickAccess::new(dir.path().into()));
        let (started_tx, started_rx) = mpsc::channel();
        let (go_tx, go_rx) = mpsc::channel();
        let host = Arc::new(SlowOnce {
            started: Mutex::new(Some(started_tx)),
            go: Mutex::new(go_rx),
            log: Mutex::new(Vec::new()),
        });
        let applier = {
            let (qa, host) = (qa.clone(), host.clone());
            std::thread::spawn(move || qa.apply(&*host))
        };
        started_rx.recv().unwrap();
        let changer = {
            let (qa, host) = (qa.clone(), host.clone());
            std::thread::spawn(move || qa.set_shortcut(&*host, Some(ALT)).unwrap())
        };
        std::thread::sleep(std::time::Duration::from_millis(50));
        go_tx.send(()).unwrap();
        applier.join().unwrap();
        assert_eq!(changer.join().unwrap().keys, ALT);
        assert_eq!(
            *host.log.lock().unwrap(),
            [
                format!("+{}", dflt()),
                format!("+{ALT}"),
                format!("-{}", dflt())
            ]
        );
    }

    // ------------------------------------------------- window helpers

    #[test]
    fn the_shortcut_shows_a_hidden_window_and_hides_a_shown_one() {
        let w = FakeSurface::default();
        assert_eq!(toggle(&w), Toggled::Shown);
        assert_eq!((w.shows.get(), w.hides.get()), (1, 0));
        assert_eq!(w.last.get(), Some(Shown { for_browser: false }));
        assert_eq!(toggle(&w), Toggled::Hidden);
        assert_eq!((w.shows.get(), w.hides.get()), (1, 1));
    }

    #[test]
    fn the_browser_unlock_opens_the_quick_window_only_when_locked() {
        use BrowserUnlockTarget::*;
        assert_eq!(browser_unlock_target(Some((true, false))), Quick);
        assert_eq!(browser_unlock_target(Some((true, true))), Nothing);
        assert_eq!(browser_unlock_target(Some((false, false))), Main);
        assert_eq!(browser_unlock_target(None), Main);
    }

    #[test]
    fn only_a_browser_opening_hides_on_the_next_unlock() {
        let dir = tempfile::tempdir().unwrap();
        let qa = QuickAccess::new(dir.path().into());
        assert!(!qa.take_browser_unlock());
        qa.note_opened(false, Shown { for_browser: true });
        assert!(qa.take_browser_unlock());
        assert!(!qa.take_browser_unlock());
        // Dismissed before unlocking.
        qa.note_opened(false, Shown { for_browser: true });
        qa.note_hidden();
        assert!(!qa.take_browser_unlock());
        // Reopened by the shortcut.
        qa.note_opened(false, Shown { for_browser: true });
        qa.note_opened(false, Shown { for_browser: false });
        assert!(!qa.take_browser_unlock());
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
        assert_eq!(
            centered((1920, 0), (2560, 1440), (640, 420)),
            (1920 + 960, 510)
        );
        assert_eq!(centered((-1920, 0), (1920, 1080), (640, 420)), (-1280, 330));
        assert_eq!(centered((100, 50), (600, 400), (640, 420)), (100, 50));
    }
}
