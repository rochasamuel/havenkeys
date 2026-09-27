# Desktop Auto-Update Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The desktop app finds new signed releases on GitHub, offers them in a banner, and on the user's click downloads, verifies, locks the vault, installs and restarts. On `.deb`/`.rpm` installs the button opens the release page instead.

**Architecture:** `tauri-plugin-updater` is driven only from Rust. A pure module (`updates.rs`) holds the settings file, the phase machine and the small rules, and is fully unit-tested. A glue module (`updater.rs`) owns the plugin, the four commands, the scheduler and the `updates://status` event. The React UI renders a banner and a Settings section from that status. The release workflow signs the update artifacts and publishes `latest.json`.

**Tech Stack:** Rust (Tauri 2, `tauri-plugin-updater` 2, tokio time), React 19 + TypeScript, vitest, GitHub Actions (`tauri-apps/tauri-action` v0.6.2 pinned), NSIS hooks.

**Spec:** `docs/superpowers/specs/2026-09-27-desktop-auto-update-design.md`

## Global Constraints

- Endpoint (exact): `https://github.com/rochasamuel/havenkeys/releases/latest/download/latest.json`
- Release page URL for "Download" (a constant in Rust, never from `latest.json`): `https://github.com/rochasamuel/havenkeys/releases/latest`
- Updater public key (exact, committed):
  `dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDFEM0FEODM5RUM4MjY3NzkKUldSNVo0THNPZGc2SGJVMnJ4UlJ2SVpQZnpyeGloMFRocnB2L1JoVzd6TzFUVmtzVjBUNUNZcGkK`
  (minisign key ID `1D3AD839EC826779`).
- Repository secrets (already set by the owner): `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Never generate, read or print a private key.
- First check 5 s after start, then every 24 h; no automatic check in debug builds.
- `updates.json` in the app data folder, `{ "autoCheck": bool }`, default `true`, mode 0600 on Unix, written atomically, unknown fields ignored on read.
- Release notes truncated to 4 000 characters and shown as plain text only.
- The webview gets no updater plugin permission. The four commands are `update_status`, `check_for_update`, `install_update`, `set_update_auto_check`.
- Order in install: download → signature verified → `state.lock(app, "update")` → install → restart. A failed download/verification never locks.
- Windows installer: NSIS, `installMode: "passive"`; `updaterJsonPreferNsis: true`.
- Releases stay drafts (`releaseDraft: true`).
- Every user-visible string in English and pt-BR (`pt-BR.ts` is typed as `Messages`).
- Rust error messages are fixed strings; errors reach the UI as `{ code, message }` (`CmdError`).
- Commits: no `Co-Authored-By` trailers (owner's rule).

## Review Focus

- **User clicks Update twice (double-click, or banner and tray at once):** the second call must be refused with `update_unavailable`, not start a second download. → Task 1 test `install_is_refused_unless_an_update_is_available`, Task 2 relies on it.
- **No network, GitHub down, or a captive portal answering with HTML during the background check:** nothing is shown; only "Check now" shows "Could not check for updates." → Task 1 test `a_background_failure_is_silent_a_manual_one_is_reported`.
- **Release notes with markup or huge text** (`<img src=x onerror=alert(1)>`, 1 MB of text, multi-byte characters at the cut): shown literally, truncated on a character boundary. → Task 1 test `notes_are_truncated_on_a_character_boundary`, Task 4 tests `passes notes through as text` and `the banner never renders HTML`.
- **`updates.json` corrupt, empty, from a newer version with extra fields, or unwritable folder:** load falls back to `autoCheck: true`; save failure returns `update_settings` without changing the in-memory setting. → Task 1 tests `settings_*`, Task 2 step on ordering.
- **Update offered while the vault is locked (unlock screen):** banner shows and installing works without unlocking; locking an already locked vault is a no-op. → Task 4 App integration + Task 8 manual check.

---

### Task 1: Pure update logic (`updates.rs`)

**Files:**
- Create: `apps/desktop/src-tauri/src/updates.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (add `mod updates;`)

**Interfaces:**
- Produces (used by Task 2 and 3):
  - `pub struct UpdateSettings { pub auto_check: bool }` with `Default` (`true`), `UpdateSettings::load(dir: &Path) -> UpdateSettings`, `UpdateSettings::save(&self, dir: &Path) -> std::io::Result<()>`
  - `pub enum Stage { Check, Install }` (serde `"check"` / `"install"`)
  - `pub enum Phase { Idle, Checking, Available { version: String, notes: String }, Downloading { version: String, downloaded: u64, total: Option<u64> }, Installing, Failed { during: Stage } }`, serde internally tagged `"phase"`, camelCase
  - `pub enum CheckOutcome { UpToDate, Found { version: String, notes: String }, Failed }`
  - `pub struct NotAvailable;`
  - `pub struct Machine` with `new()`, `phase(&self) -> &Phase`, `begin_check(&mut self) -> bool`, `finish_check(&mut self, outcome: CheckOutcome, manual: bool)`, `begin_install(&mut self) -> Result<String, NotAvailable>`, `progress(&mut self, chunk: u64, total: Option<u64>) -> bool`, `installing(&mut self)`, `install_failed(&mut self)`
  - `pub fn can_install_in_place(os: &str, appimage: bool) -> bool`, `pub fn installs_in_place() -> bool`
  - `pub const NOTES_LIMIT: usize = 4000;`

- [ ] **Step 1: Write the module skeleton with the failing tests**

Create `apps/desktop/src-tauri/src/updates.rs`:

```rust
//! What in-app updates decide, without Tauri: the setting on disk, the
//! phases an update goes through, and the small rules around them. The
//! plugin, the commands and the timer are in `updater.rs`.
//!
//! See docs/superpowers/specs/2026-09-27-desktop-auto-update-design.md.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

const FILE: &str = "updates.json";
/// Release notes longer than this are cut (in characters, not bytes).
pub const NOTES_LIMIT: usize = 4000;
/// Without a size from the server, progress is reported every this many bytes.
const UNKNOWN_SIZE_STEP: u64 = 1024 * 1024;

/// This computer's update setting. Outside the vault so it can be read
/// while the vault is locked; it holds nothing secret.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettings {
    #[serde(default = "default_auto_check")]
    pub auto_check: bool,
}

fn default_auto_check() -> bool {
    true
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self { auto_check: true }
    }
}

impl UpdateSettings {
    /// Missing, unreadable or malformed means the default. Unknown fields
    /// (a newer version's) are ignored rather than rejected.
    pub fn load(dir: &Path) -> Self {
        todo!()
    }

    /// Written to a temporary file and renamed over the old one.
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        todo!()
    }
}

/// Which step failed, so the UI can say it in its own words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Check,
    Install,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum Phase {
    Idle,
    Checking,
    Available {
        version: String,
        notes: String,
    },
    Downloading {
        version: String,
        downloaded: u64,
        total: Option<u64>,
    },
    Installing,
    Failed {
        during: Stage,
    },
}

pub enum CheckOutcome {
    UpToDate,
    Found { version: String, notes: String },
    Failed,
}

/// `install` was asked for without an update on offer.
#[derive(Debug, PartialEq, Eq)]
pub struct NotAvailable;

pub struct Machine {
    phase: Phase,
}

impl Machine {
    pub fn new() -> Self {
        Self { phase: Phase::Idle }
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    /// Starts a check unless one is running or an update is being
    /// downloaded or installed. `false` means "do nothing".
    pub fn begin_check(&mut self) -> bool {
        todo!()
    }

    /// A background check that fails is not worth interrupting anyone for;
    /// only a check the user asked for reports failure.
    pub fn finish_check(&mut self, outcome: CheckOutcome, manual: bool) {
        todo!()
    }

    /// Only from `Available`, so a second click cannot start a second
    /// download. Returns the version being installed.
    pub fn begin_install(&mut self) -> Result<String, NotAvailable> {
        todo!()
    }

    /// Adds a downloaded chunk. `true` when the change is worth telling the
    /// UI about: the whole percentage moved, or (size unknown) another MiB.
    pub fn progress(&mut self, chunk: u64, total: Option<u64>) -> bool {
        todo!()
    }

    pub fn installing(&mut self) {
        self.phase = Phase::Installing;
    }

    pub fn install_failed(&mut self) {
        self.phase = Phase::Failed {
            during: Stage::Install,
        };
    }
}

fn truncate_notes(notes: &str) -> String {
    todo!()
}

/// Whether this install can replace itself. The updater handles the
/// Windows installer, the macOS app bundle and the AppImage; a `.deb` or
/// `.rpm` install belongs to the package manager.
pub fn can_install_in_place(os: &str, appimage: bool) -> bool {
    todo!()
}

/// `can_install_in_place` for this process. The AppImage runtime sets
/// `APPIMAGE` to the image's path.
pub fn installs_in_place() -> bool {
    can_install_in_place(std::env::consts::OS, std::env::var_os("APPIMAGE").is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(version: &str) -> CheckOutcome {
        CheckOutcome::Found {
            version: version.into(),
            notes: "Fixes.".into(),
        }
    }

    #[test]
    fn settings_default_to_checking_when_the_file_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(UpdateSettings::load(dir.path()), UpdateSettings { auto_check: true });
    }

    #[test]
    fn settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        UpdateSettings { auto_check: false }.save(dir.path()).unwrap();
        assert_eq!(UpdateSettings::load(dir.path()), UpdateSettings { auto_check: false });
        UpdateSettings { auto_check: true }.save(dir.path()).unwrap();
        assert_eq!(UpdateSettings::load(dir.path()), UpdateSettings { auto_check: true });
        assert!(!dir.path().join("updates.json.tmp").exists());
    }

    #[test]
    fn settings_fall_back_to_the_default_when_corrupt_or_empty() {
        let dir = tempfile::tempdir().unwrap();
        for bytes in [&b""[..], b"{", b"null", b"[]", b"{\"autoCheck\":\"no\"}"] {
            std::fs::write(dir.path().join(FILE), bytes).unwrap();
            assert_eq!(UpdateSettings::load(dir.path()), UpdateSettings::default());
        }
    }

    #[test]
    fn settings_ignore_fields_from_a_newer_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE),
            br#"{"autoCheck":false,"channel":"beta"}"#,
        )
        .unwrap();
        assert_eq!(UpdateSettings::load(dir.path()), UpdateSettings { auto_check: false });
    }

    #[test]
    fn settings_save_fails_cleanly_when_the_folder_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("gone");
        assert!(UpdateSettings { auto_check: false }.save(&missing).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn settings_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        UpdateSettings::default().save(dir.path()).unwrap();
        let mode = std::fs::metadata(dir.path().join(FILE)).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_found_update_becomes_available() {
        let mut m = Machine::new();
        assert!(m.begin_check());
        assert_eq!(m.phase(), &Phase::Checking);
        m.finish_check(found("0.9.0"), false);
        assert_eq!(
            m.phase(),
            &Phase::Available { version: "0.9.0".into(), notes: "Fixes.".into() }
        );
    }

    #[test]
    fn up_to_date_returns_to_idle() {
        let mut m = Machine::new();
        m.begin_check();
        m.finish_check(CheckOutcome::UpToDate, true);
        assert_eq!(m.phase(), &Phase::Idle);
    }

    #[test]
    fn a_background_failure_is_silent_a_manual_one_is_reported() {
        let mut m = Machine::new();
        m.begin_check();
        m.finish_check(CheckOutcome::Failed, false);
        assert_eq!(m.phase(), &Phase::Idle);
        m.begin_check();
        m.finish_check(CheckOutcome::Failed, true);
        assert_eq!(m.phase(), &Phase::Failed { during: Stage::Check });
        // A failure does not block the next check.
        assert!(m.begin_check());
    }

    #[test]
    fn checks_do_not_overlap_or_interrupt_an_install() {
        let mut m = Machine::new();
        assert!(m.begin_check());
        assert!(!m.begin_check());
        m.finish_check(found("0.9.0"), false);
        m.begin_install().unwrap();
        assert!(!m.begin_check());
        m.installing();
        assert!(!m.begin_check());
    }

    #[test]
    fn a_new_check_is_allowed_while_an_update_is_on_offer() {
        let mut m = Machine::new();
        m.begin_check();
        m.finish_check(found("0.9.0"), false);
        assert!(m.begin_check());
        m.finish_check(found("0.9.1"), true);
        assert!(matches!(m.phase(), Phase::Available { version, .. } if version == "0.9.1"));
    }

    #[test]
    fn install_is_refused_unless_an_update_is_available() {
        let mut m = Machine::new();
        assert_eq!(m.begin_install(), Err(NotAvailable));
        m.begin_check();
        assert_eq!(m.begin_install(), Err(NotAvailable));
        m.finish_check(found("0.9.0"), false);
        assert_eq!(m.begin_install(), Ok("0.9.0".into()));
        assert_eq!(
            m.phase(),
            &Phase::Downloading { version: "0.9.0".into(), downloaded: 0, total: None }
        );
        // The second click.
        assert_eq!(m.begin_install(), Err(NotAvailable));
    }

    #[test]
    fn install_failure_is_reported_and_allows_a_new_check() {
        let mut m = Machine::new();
        m.begin_check();
        m.finish_check(found("0.9.0"), false);
        m.begin_install().unwrap();
        m.install_failed();
        assert_eq!(m.phase(), &Phase::Failed { during: Stage::Install });
        assert!(m.begin_check());
    }

    #[test]
    fn progress_reports_whole_percent_steps_only() {
        let mut m = Machine::new();
        m.begin_check();
        m.finish_check(found("0.9.0"), false);
        m.begin_install().unwrap();
        assert!(!m.progress(1, Some(1000))); // 0.1 %: still 0
        assert!(m.progress(9, Some(1000))); // 1 %
        assert!(!m.progress(5, Some(1000))); // 1.5 %
        assert!(m.progress(985, Some(1000))); // 100 %
        assert_eq!(
            m.phase(),
            &Phase::Downloading { version: "0.9.0".into(), downloaded: 1000, total: Some(1000) }
        );
    }

    #[test]
    fn progress_without_a_size_reports_each_mebibyte() {
        let mut m = Machine::new();
        m.begin_check();
        m.finish_check(found("0.9.0"), false);
        m.begin_install().unwrap();
        assert!(!m.progress(1000, None));
        assert!(m.progress(UNKNOWN_SIZE_STEP, None));
        assert!(!m.progress(10, None));
    }

    #[test]
    fn progress_outside_a_download_is_ignored() {
        let mut m = Machine::new();
        assert!(!m.progress(10, Some(10)));
        assert_eq!(m.phase(), &Phase::Idle);
    }

    #[test]
    fn notes_are_truncated_on_a_character_boundary() {
        assert_eq!(truncate_notes("short"), "short");
        let exact = "a".repeat(NOTES_LIMIT);
        assert_eq!(truncate_notes(&exact), exact);
        // Multi-byte characters straddling the limit must not panic or split.
        let long = "é".repeat(NOTES_LIMIT + 10);
        let cut = truncate_notes(&long);
        assert_eq!(cut.chars().count(), NOTES_LIMIT + 1);
        assert!(cut.ends_with('…'));
        // Markup is kept as text; escaping is the UI's job (React text nodes).
        let markup = "<img src=x onerror=alert(1)>";
        assert_eq!(truncate_notes(markup), markup);
    }

    #[test]
    fn found_notes_are_truncated() {
        let mut m = Machine::new();
        m.begin_check();
        m.finish_check(
            CheckOutcome::Found { version: "0.9.0".into(), notes: "x".repeat(1_000_000) },
            false,
        );
        let Phase::Available { notes, .. } = m.phase() else { panic!() };
        assert_eq!(notes.chars().count(), NOTES_LIMIT + 1);
    }

    #[test]
    fn in_place_install_by_platform() {
        assert!(can_install_in_place("windows", false));
        assert!(can_install_in_place("macos", false));
        assert!(can_install_in_place("linux", true));
        assert!(!can_install_in_place("linux", false));
        assert!(!can_install_in_place("freebsd", false));
        assert!(!can_install_in_place("freebsd", true));
    }

    #[test]
    fn phase_serializes_for_the_ui() {
        let json = |p: Phase| serde_json::to_value(p).unwrap();
        assert_eq!(json(Phase::Idle), serde_json::json!({ "phase": "idle" }));
        assert_eq!(
            json(Phase::Available { version: "0.9.0".into(), notes: "n".into() }),
            serde_json::json!({ "phase": "available", "version": "0.9.0", "notes": "n" })
        );
        assert_eq!(
            json(Phase::Downloading { version: "0.9.0".into(), downloaded: 5, total: None }),
            serde_json::json!({ "phase": "downloading", "version": "0.9.0", "downloaded": 5, "total": null })
        );
        assert_eq!(
            json(Phase::Failed { during: Stage::Install }),
            serde_json::json!({ "phase": "failed", "during": "install" })
        );
    }
}
```

Add `mod updates;` to `apps/desktop/src-tauri/src/lib.rs` in the alphabetical `mod` list (after `mod tray;`). Add `#[allow(dead_code)]` on the line above `mod updates;` for now; Task 2 removes it.

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test -p havenkeys-desktop updates::`
Expected: compiles, tests FAIL with `not yet implemented` panics.

- [ ] **Step 3: Implement**

Replace each `todo!()`:

```rust
    pub fn load(dir: &Path) -> Self {
        std::fs::read(dir.join(FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let bytes = serde_json::to_vec(self).map_err(std::io::Error::other)?;
        let tmp = dir.join("updates.json.tmp");
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
```

```rust
    pub fn begin_check(&mut self) -> bool {
        match self.phase {
            Phase::Checking | Phase::Downloading { .. } | Phase::Installing => false,
            _ => {
                self.phase = Phase::Checking;
                true
            }
        }
    }

    pub fn finish_check(&mut self, outcome: CheckOutcome, manual: bool) {
        self.phase = match outcome {
            CheckOutcome::Found { version, notes } => Phase::Available {
                version,
                notes: truncate_notes(&notes),
            },
            CheckOutcome::UpToDate => Phase::Idle,
            CheckOutcome::Failed if manual => Phase::Failed { during: Stage::Check },
            CheckOutcome::Failed => Phase::Idle,
        };
    }

    pub fn begin_install(&mut self) -> Result<String, NotAvailable> {
        let Phase::Available { version, .. } = &self.phase else {
            return Err(NotAvailable);
        };
        let version = version.clone();
        self.phase = Phase::Downloading {
            version: version.clone(),
            downloaded: 0,
            total: None,
        };
        Ok(version)
    }

    pub fn progress(&mut self, chunk: u64, total: Option<u64>) -> bool {
        let Phase::Downloading { downloaded, total: known, .. } = &mut self.phase else {
            return false;
        };
        let before = *downloaded;
        *downloaded = before.saturating_add(chunk);
        *known = total;
        match total {
            Some(size) if size > 0 => {
                let percent = |n: u64| n.saturating_mul(100) / size;
                percent(*downloaded) != percent(before)
            }
            _ => *downloaded / UNKNOWN_SIZE_STEP != before / UNKNOWN_SIZE_STEP,
        }
    }
```

```rust
fn truncate_notes(notes: &str) -> String {
    match notes.char_indices().nth(NOTES_LIMIT) {
        None => notes.to_owned(),
        Some((cut, _)) => format!("{}…", &notes[..cut]),
    }
}

pub fn can_install_in_place(os: &str, appimage: bool) -> bool {
    match os {
        "windows" | "macos" => true,
        "linux" => appimage,
        _ => false,
    }
}
```

- [ ] **Step 4: Run the tests to confirm they pass**

Run: `cargo test -p havenkeys-desktop updates::`
Expected: all `updates::tests::*` PASS.

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p havenkeys-desktop --all-targets -- -D warnings`
Expected: no warnings. (If `Machine::new` triggers `new_without_default`, add `impl Default for Machine { fn default() -> Self { Self::new() } }`.)

```bash
git add apps/desktop/src-tauri/src/updates.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): update phases and the update setting file"
```

---

### Task 2: Updater plugin, commands, scheduler and API bindings

The command surface is declared in four places that `apps/desktop/src/lib/commands.test.ts` checks agree: `build.rs`, `lib.rs` `generate_handler!`, `capabilities/main.json`, `src/lib/api.ts`. The error table test (`src/i18n/errors.test.ts`) checks every `code: "...", message: "..."` in Rust appears in `en.ts`/`pt-BR.ts` with Rust's exact English. All of these change together in this task.

**Files:**
- Create: `apps/desktop/src-tauri/src/updater.rs`
- Modify: `apps/desktop/src-tauri/Cargo.toml` (dependency)
- Modify: `apps/desktop/src-tauri/tauri.conf.json` (`plugins.updater`)
- Modify: `apps/desktop/src-tauri/build.rs` (`COMMANDS`)
- Modify: `apps/desktop/src-tauri/capabilities/main.json`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (module, plugin, managed state, schedule, handlers)
- Modify: `apps/desktop/src/lib/types.ts`, `apps/desktop/src/lib/api.ts`
- Modify: `apps/desktop/src/i18n/en.ts`, `apps/desktop/src/i18n/pt-BR.ts` (error codes)

**Interfaces:**
- Consumes: everything Task 1 produces; `AppState::lock(&self, app: &AppHandle, reason: &'static str)`, `AppState::touch()`, `AppState::vault()`, `AppState::data_dir()`, `CmdError`, `CmdResult`, `CmdError::internal()`, `CmdError::open_website()` (existing).
- Produces:
  - Rust: `pub struct Updates` (managed state), `pub const STATUS_EVENT: &str = "updates://status"`, `pub const RELEASES_URL: &str`, `pub fn schedule(app: AppHandle)`, `pub struct UpdateStatus` (serialized as `{ phase, …phase fields, autoCheck, canInstallInPlace, currentVersion }`), commands `update_status`, `check_for_update`, `install_update`, `set_update_auto_check(enabled: bool)`. A hook `crate::tray::set_update_available(app: &AppHandle, available: bool)` is called from `publish` — Task 3 implements it; in this task add a no-op stub in `tray.rs` with that exact signature.
  - TS (`types.ts`): `UpdatePhase`, `UpdateStatus`.
  - TS (`api.ts`): `api.updateStatus(): Promise<UpdateStatus>`, `api.checkForUpdate(): Promise<UpdateStatus>`, `api.installUpdate(): Promise<void>`, `api.setUpdateAutoCheck(enabled: boolean): Promise<UpdateStatus>`, `api.onUpdateStatus(handler: (s: UpdateStatus) => void): Promise<UnlistenFn>`.

- [ ] **Step 1: Add the dependency and plugin config**

In `apps/desktop/src-tauri/Cargo.toml`, after the `tauri-plugin-opener` entry:

```toml
# In-app updates (updater.rs), driven from Rust only: the capability grants
# the renderer none of the plugin's commands. Verifies each download against
# the minisign public key in tauri.conf.json before it can be installed.
tauri-plugin-updater = "2"
```

In `apps/desktop/src-tauri/tauri.conf.json`, add a top-level `"plugins"` key (sibling of `"bundle"`):

```json
  "plugins": {
    "updater": {
      "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDFEM0FEODM5RUM4MjY3NzkKUldSNVo0THNPZGc2SGJVMnJ4UlJ2SVpQZnpyeGloMFRocnB2L1JoVzd6TzFUVmtzVjBUNUNZcGkK",
      "endpoints": [
        "https://github.com/rochasamuel/havenkeys/releases/latest/download/latest.json"
      ],
      "windows": {
        "installMode": "passive"
      }
    }
  }
```

Run: `cargo build -p havenkeys-desktop`
Expected: builds (downloads the plugin).

- [ ] **Step 2: Confirm the plugin verifies the signature inside `download`**

The install order in this plan depends on `Update::download` returning only verified bytes.

Run: `grep -n "fn download\b\|fn download<\|verify_signature" ~/.cargo/registry/src/*/tauri-plugin-updater-2*/src/updater.rs`
Expected: `verify_signature(...)` is called inside `download` (before it returns the `Vec<u8>`). If it is only called in `install`, stop and report: the lock must then move after `install`'s verification, and the spec's §3.4 order needs revisiting.

- [ ] **Step 3: Write `updater.rs`**

Create `apps/desktop/src-tauri/src/updater.rs`:

```rust
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
```

Remove the `#[allow(dead_code)]` above `mod updates;` in `lib.rs`.

In `apps/desktop/src-tauri/src/tray.rs`, add the stub Task 3 replaces:

```rust
/// Show or hide the tray's "Update available" item. (Task 3.)
pub fn set_update_available(_app: &AppHandle, _available: bool) {}
```

- [ ] **Step 4: Wire it into `lib.rs`, `build.rs` and the capability**

`apps/desktop/src-tauri/src/lib.rs`:
- Add `mod updater;` after `mod tray;` (keep `mod updates;` after it).
- In the builder chain, after `.plugin(tauri_plugin_dialog::init())`:

```rust
        // Used from Rust only (updater.rs). The capability grants the
        // renderer none of the plugin's commands.
        .plugin(tauri_plugin_updater::Builder::new().build())
```

- In `setup`, right after `tray::install(app)?;`:

```rust
            // In-app updates: this computer's setting and the daily check.
            app.manage(updater::Updates::new(dir.clone()));
            updater::schedule(app.handle().clone());
```

- In `generate_handler![…]`, after `tray::set_ui_language,`:

```rust
            updater::update_status,
            updater::check_for_update,
            updater::install_update,
            updater::set_update_auto_check,
```

`apps/desktop/src-tauri/build.rs` — append to `COMMANDS` after `"set_ui_language",`:

```rust
    "update_status",
    "check_for_update",
    "install_update",
    "set_update_auto_check",
```

`apps/desktop/src-tauri/capabilities/main.json` — append to `permissions` (keep the existing order; add at the end):

```json
    "allow-update-status",
    "allow-check-for-update",
    "allow-install-update",
    "allow-set-update-auto-check"
```

Also update that file's `"description"` from `"… No plugins, no filesystem, no shell, no network."` to `"… No plugin commands, no filesystem, no shell, no network. (Rust itself checks GitHub for signed updates; see updater.rs.)"`.

Run: `cargo build -p havenkeys-desktop && cargo clippy -p havenkeys-desktop --all-targets -- -D warnings`
Expected: builds, no warnings.

- [ ] **Step 5: TypeScript types and API wrappers**

Append to `apps/desktop/src/lib/types.ts`:

```ts
/** Where an update stands (Rust `updates::Phase`). Carries no secrets. */
export type UpdatePhase =
  | { phase: "idle" }
  | { phase: "checking" }
  | { phase: "available"; version: string; notes: string }
  | { phase: "downloading"; version: string; downloaded: number; total: number | null }
  | { phase: "installing" }
  | { phase: "failed"; during: "check" | "install" };

export type UpdateStatus = UpdatePhase & {
  autoCheck: boolean;
  /** False for a .deb/.rpm install: the button opens the release page instead. */
  canInstallInPlace: boolean;
  currentVersion: string;
};
```

In `apps/desktop/src/lib/api.ts`, add `UpdateStatus` to the type import list, and add inside `api` after `setUiLanguage`:

```ts
  /** In-app updates. Rust talks to GitHub; the UI only sees this status. */
  updateStatus: () => call<UpdateStatus>("update_status"),
  /** "Check now". A failed check shows in the status (`failed`/`check`), not as a rejection. */
  checkForUpdate: () => call<UpdateStatus>("check_for_update"),
  /** Download, verify, lock, install and restart — or, for a .deb/.rpm install, open the release page. */
  installUpdate: () => call<void>("install_update"),
  setUpdateAutoCheck: (enabled: boolean) => call<UpdateStatus>("set_update_auto_check", { enabled }),
```

and after `onRemoved`:

```ts
  onUpdateStatus: (handler: (status: UpdateStatus) => void): Promise<UnlistenFn> =>
    listen<UpdateStatus>("updates://status", (e) => handler(e.payload)),
```

- [ ] **Step 6: Error codes in both languages**

In `apps/desktop/src/i18n/en.ts`, in the error-code union `ErrorCode` add `| "update_unavailable" | "update_failed" | "update_settings"`, and in the `codes` table (next to `autostart`):

```ts
  update_unavailable: "There is no update to install.",
  update_failed: "The update could not be installed. Try again later.",
  update_settings: "Could not save the update setting.",
```

In `apps/desktop/src/i18n/pt-BR.ts`, in its `codes` table:

```ts
  update_unavailable: "Não há atualização para instalar.",
  update_failed: "Não foi possível instalar a atualização. Tente novamente mais tarde.",
  update_settings: "Não foi possível salvar a configuração de atualizações.",
```

- [ ] **Step 7: Run the cross-file tests**

Run: `pnpm --filter @havenkeys/desktop test && pnpm --filter @havenkeys/desktop typecheck`
Expected: PASS, including `commands.test.ts` (four places agree) and `errors.test.ts` (three new codes, English identical to Rust).

Run: `cargo test -p havenkeys-desktop`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add apps/desktop/src-tauri apps/desktop/src/lib/types.ts apps/desktop/src/lib/api.ts apps/desktop/src/i18n/en.ts apps/desktop/src/i18n/pt-BR.ts Cargo.lock
git commit -m "feat(desktop): check for and install signed updates from Rust"
```

---

### Task 3: Tray "Update available" item

**Files:**
- Modify: `apps/desktop/src-tauri/src/tray.rs`

**Interfaces:**
- Consumes: `set_update_available(app: &AppHandle, available: bool)` is already called by `updater::publish` (Task 2).
- Produces: the real implementation; `UiLanguage::labels()` now returns `[&'static str; 4]` in the order `open`, `lock`, `quit`, `update`.

- [ ] **Step 1: Update the label test first**

In `tray.rs` tests, replace `the_tray_labels_come_from_the_fixed_table` with:

```rust
    #[test]
    fn the_tray_labels_come_from_the_fixed_table() {
        assert_eq!(
            UiLanguage::En.labels(),
            ["Open HavenKeys", "Lock", "Quit HavenKeys", "Update available"]
        );
        assert_eq!(
            UiLanguage::PtBr.labels(),
            ["Abrir HavenKeys", "Bloquear", "Sair do HavenKeys", "Atualização disponível"]
        );
    }
```

Run: `cargo test -p havenkeys-desktop tray::`
Expected: FAIL to compile (array length 3 vs 4).

- [ ] **Step 2: Implement**

In `tray.rs`:

1. `labels()` doc: `/// Labels for the menu items \`open\`, \`lock\`, \`quit\` and \`update\`, in that order.` and return `[&'static str; 4]`:

```rust
            Self::En => ["Open HavenKeys", "Lock", "Quit HavenKeys", "Update available"],
            Self::PtBr => ["Abrir HavenKeys", "Bloquear", "Sair do HavenKeys", "Atualização disponível"],
```

2. Extend `TrayItems` and import `Mutex`:

```rust
use std::sync::Mutex;

struct TrayItems {
    open: MenuItem<Wry>,
    lock: MenuItem<Wry>,
    quit: MenuItem<Wry>,
    /// Put at the top of the menu only while an update is on offer.
    update: MenuItem<Wry>,
    menu: Menu<Wry>,
    update_shown: Mutex<bool>,
}
```

3. `set_ui_language`: destructure four labels and include `(&items.update, update)` in the loop:

```rust
    let [open, lock, quit, update] = UiLanguage::parse(&lang)?.labels();
    if let Some(items) = app.try_state::<TrayItems>() {
        for (item, label) in [
            (&items.open, open),
            (&items.lock, lock),
            (&items.quit, quit),
            (&items.update, update),
        ] {
            item.set_text(label).map_err(|_| CmdError::internal())?;
        }
    }
```

4. Replace the Task 2 stub:

```rust
/// Show or hide the tray's "Update available" item. It carries no version
/// or notes: choosing it only opens the window, where the banner is.
pub fn set_update_available(app: &AppHandle, available: bool) {
    let Some(items) = app.try_state::<TrayItems>() else {
        return;
    };
    let Ok(mut shown) = items.update_shown.lock() else {
        return;
    };
    if *shown == available {
        return;
    }
    let changed = if available {
        items.menu.insert(&items.update, 0)
    } else {
        items.menu.remove(&items.update)
    };
    if changed.is_ok() {
        *shown = available;
    }
}
```

5. In `install`: build the item and keep the menu:

```rust
    let [open_label, lock_label, quit_label, update_label] = UiLanguage::En.labels();
    // …existing open/lock/quit items…
    let update = MenuItem::with_id(app, "update", update_label, true, None::<&str>)?;
```

add `"update" => show_main_window(app),` to `on_menu_event`, and at the end:

```rust
    app.manage(TrayItems {
        open,
        lock,
        quit,
        update,
        menu,
        update_shown: Mutex::new(false),
    });
```

(`menu` is passed to the builder by reference already — `.menu(&menu)` — so it is still available to move here.)

- [ ] **Step 3: Test, lint, commit**

Run: `cargo test -p havenkeys-desktop tray:: && cargo clippy -p havenkeys-desktop --all-targets -- -D warnings`
Expected: PASS, no warnings.

```bash
git add apps/desktop/src-tauri/src/tray.rs
git commit -m "feat(desktop): tray item while an update is available"
```

---

### Task 4: Update banner

**Files:**
- Create: `apps/desktop/src/lib/updates.ts`, `apps/desktop/src/lib/updates.test.ts`
- Create: `apps/desktop/src/components/UpdateBanner.tsx`, `apps/desktop/src/components/UpdateBanner.test.ts`
- Modify: `apps/desktop/src/lib/hooks.ts` (`useUpdateStatus`)
- Modify: `apps/desktop/src/App.tsx`, `apps/desktop/src/styles.css`
- Modify: `apps/desktop/src/i18n/en.ts`, `apps/desktop/src/i18n/pt-BR.ts` (`updates` group)

**Interfaces:**
- Consumes: `UpdateStatus` (types.ts), `api.updateStatus`, `api.onUpdateStatus`, `api.installUpdate`, `api.checkForUpdate` (Task 2).
- Produces:
  - `bannerFor(status: UpdateStatus | null, dismissed: string | null): Banner` and `type Banner` in `lib/updates.ts`
  - `useUpdateStatus(): UpdateStatus | null` in `lib/hooks.ts` (used by Task 5)
  - `t.updates.*` strings (used by Task 5): `available(v)`, `whatsNew`, `hideNotes`, `update`, `download`, `later`, `restartNote`, `downloading(v, pct)`, `installing`, `failed`, `tryAgain`, `title`, `autoCheck`, `checkNow`, `checking`, `upToDate`, `checkFailed`, `version(v)`, `note`, `manualNote`

- [ ] **Step 1: Write the failing model tests**

Create `apps/desktop/src/lib/updates.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import type { UpdatePhase, UpdateStatus } from "./types";
import { bannerFor } from "./updates";

const base = { autoCheck: true, canInstallInPlace: true, currentVersion: "0.9.0" };
const s = (phase: UpdatePhase, extra: Partial<UpdateStatus> = {}): UpdateStatus =>
  ({ ...base, ...phase, ...extra }) as UpdateStatus;
const available = s({ phase: "available", version: "0.9.1", notes: "Fixes." });

describe("bannerFor", () => {
  it("shows nothing before the status arrives, while idle or checking", () => {
    expect(bannerFor(null, null)).toEqual({ kind: "hidden" });
    expect(bannerFor(s({ phase: "idle" }), null)).toEqual({ kind: "hidden" });
    expect(bannerFor(s({ phase: "checking" }), null)).toEqual({ kind: "hidden" });
  });

  it("offers an available update, installing in place when it can", () => {
    expect(bannerFor(available, null)).toEqual({
      kind: "available",
      version: "0.9.1",
      notes: "Fixes.",
      action: "update",
    });
  });

  it("offers a download for .deb/.rpm installs", () => {
    const deb = { ...available, canInstallInPlace: false };
    expect(bannerFor(deb, null)).toMatchObject({ kind: "available", action: "download" });
  });

  it("stays hidden once that version is dismissed, but not for a newer one", () => {
    expect(bannerFor(available, "0.9.1")).toEqual({ kind: "hidden" });
    expect(bannerFor(available, "0.9.0")).toMatchObject({ kind: "available" });
  });

  it("passes notes through as text", () => {
    const markup = "<img src=x onerror=alert(1)>";
    const withMarkup = s({ phase: "available", version: "0.9.1", notes: markup });
    expect(bannerFor(withMarkup, null)).toMatchObject({ notes: markup });
  });

  it("shows download progress, capped at 100, or none when the size is unknown", () => {
    expect(bannerFor(s({ phase: "downloading", version: "0.9.1", downloaded: 50, total: 200 }), null)).toEqual({
      kind: "downloading",
      version: "0.9.1",
      percent: 25,
    });
    expect(
      bannerFor(s({ phase: "downloading", version: "0.9.1", downloaded: 300, total: 200 }), null),
    ).toMatchObject({ percent: 100 });
    expect(
      bannerFor(s({ phase: "downloading", version: "0.9.1", downloaded: 5, total: null }), null),
    ).toMatchObject({ percent: null });
    expect(
      bannerFor(s({ phase: "downloading", version: "0.9.1", downloaded: 5, total: 0 }), null),
    ).toMatchObject({ percent: null });
  });

  it("shows progress even if the version was dismissed earlier", () => {
    expect(
      bannerFor(s({ phase: "downloading", version: "0.9.1", downloaded: 0, total: 10 }), "0.9.1"),
    ).toMatchObject({ kind: "downloading" });
  });

  it("shows installing and install failures; a failed check stays out of the banner", () => {
    expect(bannerFor(s({ phase: "installing" }), null)).toEqual({ kind: "installing" });
    expect(bannerFor(s({ phase: "failed", during: "install" }), null)).toEqual({ kind: "failed" });
    expect(bannerFor(s({ phase: "failed", during: "check" }), null)).toEqual({ kind: "hidden" });
  });
});
```

Create `apps/desktop/src/components/UpdateBanner.test.ts`:

```ts
// Release notes come from a downloaded file. They must only ever reach the
// page as React text, which escapes them.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

const source = readFileSync(fileURLToPath(new URL("./UpdateBanner.tsx", import.meta.url)), "utf8");

describe("UpdateBanner", () => {
  it("the banner never renders HTML", () => {
    expect(source).not.toMatch(/dangerouslySetInnerHTML|innerHTML|outerHTML|insertAdjacentHTML/);
  });
});
```

Run: `pnpm --filter @havenkeys/desktop test -- updates UpdateBanner`
Expected: FAIL (modules missing).

- [ ] **Step 2: Implement the model**

Create `apps/desktop/src/lib/updates.ts`:

```ts
// What the update banner shows for a given status. The notes are passed on
// as a string for React to render as text; nothing here builds markup.

import type { UpdateStatus } from "./types";

export type Banner =
  | { kind: "hidden" }
  | { kind: "available"; version: string; notes: string; action: "update" | "download" }
  | { kind: "downloading"; version: string; percent: number | null }
  | { kind: "installing" }
  | { kind: "failed" };

/**
 * `dismissed` is the version the user chose "Later" for in this run of the
 * app. A failed *check* is reported in Settings, where it was asked for.
 */
export function bannerFor(status: UpdateStatus | null, dismissed: string | null): Banner {
  if (!status) return { kind: "hidden" };
  switch (status.phase) {
    case "available":
      if (status.version === dismissed) return { kind: "hidden" };
      return {
        kind: "available",
        version: status.version,
        notes: status.notes,
        action: status.canInstallInPlace ? "update" : "download",
      };
    case "downloading": {
      const percent =
        status.total !== null && status.total > 0
          ? Math.min(100, Math.floor((status.downloaded * 100) / status.total))
          : null;
      return { kind: "downloading", version: status.version, percent };
    }
    case "installing":
      return { kind: "installing" };
    case "failed":
      return status.during === "install" ? { kind: "failed" } : { kind: "hidden" };
    default:
      return { kind: "hidden" };
  }
}
```

Run: `pnpm --filter @havenkeys/desktop test -- updates`
Expected: `updates.test.ts` PASS (`UpdateBanner.test.ts` still fails).

- [ ] **Step 3: Strings**

In `apps/desktop/src/i18n/en.ts`, add a top-level group (before `errors`):

```ts
  updates: {
    available: (version: string) => `HavenKeys ${version} is available.`,
    whatsNew: "What’s new",
    hideNotes: "Hide notes",
    update: "Update",
    download: "Download",
    later: "Later",
    restartNote: "Updating locks HavenKeys and restarts it.",
    downloading: (version: string, percent: number | null) =>
      percent === null ? `Downloading HavenKeys ${version}…` : `Downloading HavenKeys ${version}… ${percent}%`,
    installing: "Installing the update…",
    failed: "The update failed. Try again later.",
    tryAgain: "Try again",
    title: "Updates",
    autoCheck: "Check for updates automatically",
    checkNow: "Check now",
    checking: "Checking…",
    upToDate: "HavenKeys is up to date.",
    checkFailed: "Could not check for updates.",
    version: (version: string) => `Version ${version}`,
    note: "HavenKeys asks GitHub, where its releases are published, whether a newer version exists. Updates are signed, and nothing is installed until you choose Update.",
    manualNote: "This copy was installed from a .deb or .rpm package. New versions are downloaded from the release page.",
  },
```

In `apps/desktop/src/i18n/pt-BR.ts`, the same group:

```ts
  updates: {
    available: (version: string) => `O HavenKeys ${version} está disponível.`,
    whatsNew: "Novidades",
    hideNotes: "Ocultar novidades",
    update: "Atualizar",
    download: "Baixar",
    later: "Depois",
    restartNote: "Atualizar bloqueia o HavenKeys e o reinicia.",
    downloading: (version: string, percent: number | null) =>
      percent === null ? `Baixando o HavenKeys ${version}…` : `Baixando o HavenKeys ${version}… ${percent}%`,
    installing: "Instalando a atualização…",
    failed: "A atualização falhou. Tente novamente mais tarde.",
    tryAgain: "Tentar novamente",
    title: "Atualizações",
    autoCheck: "Procurar atualizações automaticamente",
    checkNow: "Procurar agora",
    checking: "Procurando…",
    upToDate: "O HavenKeys está atualizado.",
    checkFailed: "Não foi possível procurar atualizações.",
    version: (version: string) => `Versão ${version}`,
    note: "O HavenKeys pergunta ao GitHub, onde suas versões são publicadas, se existe uma versão mais nova. As atualizações são assinadas, e nada é instalado até você escolher Atualizar.",
    manualNote: "Esta cópia foi instalada por um pacote .deb ou .rpm. Novas versões são baixadas pela página de versões.",
  },
```

- [ ] **Step 4: Hook and component**

Append to `apps/desktop/src/lib/hooks.ts` (add missing imports — `useEffect`, `useState` from `react`, `api` from `./api`, `UpdateStatus` from `./types` — only if not already imported):

```ts
/** The in-app update status, kept current by Rust's `updates://status` event. */
export function useUpdateStatus(): UpdateStatus | null {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  useEffect(() => {
    let live = true;
    api.updateStatus().then(
      (s) => {
        if (live) setStatus(s);
      },
      () => undefined,
    );
    const unlisten = api.onUpdateStatus((s) => setStatus(s));
    return () => {
      live = false;
      void unlisten.then((f) => f());
    };
  }, []);
  return status;
}
```

Create `apps/desktop/src/components/UpdateBanner.tsx`:

```tsx
import { useState } from "react";
import { api } from "../lib/api";
import { useUpdateStatus } from "../lib/hooks";
import { bannerFor } from "../lib/updates";
import { useI18n } from "../i18n/context";

/**
 * "HavenKeys 0.9.1 is available" at the top of the window, locked or not.
 * Release notes are rendered as text only (see UpdateBanner.test.ts).
 * Failures are reported through the status, so rejected calls are ignored.
 */
export function UpdateBanner({
  dismissed,
  onDismiss,
}: {
  /** The version the user chose "Later" for; kept by App so unlocking does not bring it back. */
  dismissed: string | null;
  onDismiss: (version: string) => void;
}) {
  const { t } = useI18n();
  const status = useUpdateStatus();
  const [showNotes, setShowNotes] = useState(false);
  const banner = bannerFor(status, dismissed);

  switch (banner.kind) {
    case "hidden":
      return null;
    case "available":
      return (
        <div className="banner update-banner" role="status">
          <span>{t.updates.available(banner.version)}</span>
          {banner.action === "update" && <span className="update-hint">{t.updates.restartNote}</span>}
          {banner.notes && (
            <button className="btn btn-quiet" type="button" onClick={() => setShowNotes((v) => !v)}>
              {showNotes ? t.updates.hideNotes : t.updates.whatsNew}
            </button>
          )}
          <button className="btn btn-quiet" type="button" onClick={() => onDismiss(banner.version)}>
            {t.updates.later}
          </button>
          <button className="btn" type="button" onClick={() => void api.installUpdate().catch(() => undefined)}>
            {banner.action === "update" ? t.updates.update : t.updates.download}
          </button>
          {showNotes && banner.notes && <p className="update-notes">{banner.notes}</p>}
        </div>
      );
    case "downloading":
      return (
        <div className="banner update-banner" role="status">
          <span>{t.updates.downloading(banner.version, banner.percent)}</span>
          <progress className="update-progress" max={100} value={banner.percent ?? undefined} />
        </div>
      );
    case "installing":
      return (
        <div className="banner update-banner" role="status">
          {t.updates.installing}
        </div>
      );
    case "failed":
      return (
        <div className="banner banner-warn update-banner" role="alert">
          <span>{t.updates.failed}</span>
          <button className="btn btn-quiet" type="button" onClick={() => void api.checkForUpdate().catch(() => undefined)}>
            {t.updates.tryAgain}
          </button>
        </div>
      );
  }
}
```

- [ ] **Step 5: Place it in the app and style it**

In `apps/desktop/src/App.tsx`, import `UpdateBanner` from `./components/UpdateBanner`. The locked and unlocked screens mount separate banners, so "Later" is remembered in `App` (for this run of the app only). Add next to the other `useState` calls:

```tsx
  // The update version the user chose "Later" for, until the app restarts.
  const [dismissedUpdate, setDismissedUpdate] = useState<string | null>(null);
```

and define once, before the returns:

```tsx
  const updateBanner = <UpdateBanner dismissed={dismissedUpdate} onDismiss={setDismissedUpdate} />;
```

Then:

Replace the locked branch:

```tsx
  if (!unlocked) {
    return (
      <div className="app-shell">
        {updateBanner}
        <UnlockScreen
          key={session}
          lockReason={lockReason}
          needsSecretKey={device?.needsSecretKey ?? false}
          onUnlocked={(s) => {
            setLockReason(null);
            setStatus(s);
          }}
        />
      </div>
    );
  }
```

and in the unlocked return, add `{updateBanner}` as the first child of `<div className="app-shell">`.

Append to `apps/desktop/src/styles.css` (after the `.banner-warn` rule):

```css
.app-shell > .unlock {
  flex: 1;
  min-height: 0;
}

.update-banner {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 6px 10px;
  padding: 8px 14px;
}

.update-hint {
  font-weight: 400;
  opacity: 0.8;
}

.update-progress {
  width: 140px;
}

/* Release notes: plain text from the release, kept short and scrollable. */
.update-notes {
  flex-basis: 100%;
  max-height: 160px;
  overflow: auto;
  margin: 4px 0 0;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  text-align: left;
  font-weight: 400;
}
```

- [ ] **Step 6: Tests, typecheck, look at it**

Run: `pnpm --filter @havenkeys/desktop test && pnpm --filter @havenkeys/desktop typecheck`
Expected: PASS (the `pt-BR.ts` typecheck proves both languages have every string).

Visual check (debug build never checks by itself, so fake a status): temporarily change `bannerFor`'s argument in `UpdateBanner.tsx` to use `{ phase: "available", version: "9.9.9", notes: "Line one\n<b>not bold</b>", autoCheck: true, canInstallInPlace: true, currentVersion: "0.9.0" }` instead of `status`, run `pnpm --filter @havenkeys/desktop tauri dev`, confirm on the unlock screen and the vault screen that the banner shows, `<b>` appears literally, "What's new" expands, "Later" hides it, and the unlock form is not pushed off-screen. **Revert the temporary change** (`git diff apps/desktop/src/components/UpdateBanner.tsx` shows only the intended file).

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src
git commit -m "feat(desktop): update banner on the unlock and vault screens"
```

---

### Task 5: Settings → Updates and the real version in About

**Files:**
- Create: `apps/desktop/src/views/UpdatesSection.tsx`
- Modify: `apps/desktop/src/views/SettingsView.tsx`

**Interfaces:**
- Consumes: `useUpdateStatus()` (Task 4), `api.checkForUpdate`, `api.setUpdateAutoCheck` (Task 2), `t.updates.*` (Task 4), `Switch`, `useToast`, `errorMessage`.
- Produces: `UpdatesSection` component; `SettingsView` About line shows `status.currentVersion`.

- [ ] **Step 1: Write the section**

Create `apps/desktop/src/views/UpdatesSection.tsx`:

```tsx
import { useState } from "react";
import { api } from "../lib/api";
import { useUpdateStatus } from "../lib/hooks";
import { Switch } from "../components/Switch";
import { useToast } from "../components/Toast";
import { useI18n } from "../i18n/context";
import { errorMessage } from "../i18n/errors";

/** Settings → Updates: this computer's automatic check, and "Check now". */
export function UpdatesSection() {
  const { t } = useI18n();
  const toast = useToast();
  const status = useUpdateStatus();
  // Only a check asked for here says "up to date" or "could not check".
  const [asked, setAsked] = useState(false);

  if (!status) return null;

  async function setAutoCheck(enabled: boolean) {
    try {
      await api.setUpdateAutoCheck(enabled);
      toast(t.settings.saved);
    } catch (e) {
      toast(errorMessage(e, t, t.settings.saveFailed), "error");
    }
  }

  async function checkNow() {
    setAsked(true);
    await api.checkForUpdate().catch(() => undefined);
  }

  const result = !asked
    ? null
    : status.phase === "checking"
      ? t.updates.checking
      : status.phase === "idle"
        ? t.updates.upToDate
        : status.phase === "failed" && status.during === "check"
          ? t.updates.checkFailed
          : null;

  return (
    <div className="settings-block">
      <h3 className="group-title">{t.updates.title}</h3>
      <div className="group">
        <div className="row">
          <span className="row-label-inline">{t.updates.autoCheck}</span>
          <Switch
            label={t.updates.autoCheck}
            checked={status.autoCheck}
            onChange={(checked) => void setAutoCheck(checked)}
          />
        </div>
        <div className="row">
          <span className="row-label-inline">{t.updates.version(status.currentVersion)}</span>
          <button
            className="btn btn-quiet"
            type="button"
            disabled={status.phase === "checking" || status.phase === "downloading" || status.phase === "installing"}
            onClick={() => void checkNow()}
          >
            {t.updates.checkNow}
          </button>
        </div>
      </div>
      {result && (
        <p className="group-note" role="status">
          {result}
        </p>
      )}
      <p className="group-note">{status.canInstallInPlace ? t.updates.note : t.updates.manualNote}</p>
    </div>
  );
}
```

- [ ] **Step 2: Use it in Settings and fix About**

In `apps/desktop/src/views/SettingsView.tsx`:
- import `UpdatesSection` from `./UpdatesSection` and `useUpdateStatus` from `../lib/hooks`;
- inside `SettingsView`, add `const updateStatus = useUpdateStatus();`;
- render `<UpdatesSection />` right before the About block;
- replace `{t.settings.aboutText("0.8.0")}` with:

```tsx
        {updateStatus && <p className="group-note">{t.settings.aboutText(updateStatus.currentVersion)}</p>}
```

(removing the old `<p>` line).

- [ ] **Step 3: Test, typecheck, look at it, commit**

Run: `pnpm --filter @havenkeys/desktop test && pnpm --filter @havenkeys/desktop typecheck`
Expected: PASS.

Run `pnpm --filter @havenkeys/desktop tauri dev`, unlock, open Settings: the Updates group shows the switch (on), "Version 0.8.0" (or the current version), and "Check now". Click "Check now": "Checking…" then either "HavenKeys is up to date." or — while no release has `latest.json` yet — "Could not check for updates." (expected until Task 8). Toggle the switch off and on; confirm `updates.json` in the app data folder reads `{"autoCheck":false}` then `{"autoCheck":true}`. About shows the real version.

```bash
git add apps/desktop/src/views
git commit -m "feat(desktop): Settings → Updates, and About shows the running version"
```

---

### Task 6: Release pipeline and the Windows installer hook

**Files:**
- Modify: `apps/desktop/src-tauri/tauri.bundle.conf.json`
- Modify: `apps/desktop/src-tauri/tauri.conf.json` (NSIS hook)
- Create: `apps/desktop/src-tauri/windows/hooks.nsh`
- Modify: `.github/workflows/release.yml`

**Interfaces:**
- Consumes: repository secrets `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
- Produces: releases that carry `.sig` files and a merged `latest.json` with `windows-x86_64` (NSIS), `darwin-aarch64`, `darwin-x86_64` and `linux-x86_64` (AppImage) entries.

- [ ] **Step 1: Updater artifacts only in CI builds**

`createUpdaterArtifacts` makes `tauri build` require the private key. It goes in the CI-only overlay so local builds keep working. Replace `apps/desktop/src-tauri/tauri.bundle.conf.json` with:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "bundle": {
    "externalBin": ["binaries/havenkeys-native-host"],
    "createUpdaterArtifacts": true
  }
}
```

- [ ] **Step 2: NSIS hook that frees the native host**

Create `apps/desktop/src-tauri/windows/hooks.nsh`:

```nsis
; Browsers keep havenkeys-native-host.exe running (it waits for the app to
; come back), and Windows cannot overwrite a running program. End it before
; the files are copied; the extension starts a new one on its next request.
; taskkill only reaches this user's processes without elevation, which is
; where a per-user install's browsers run.
!macro NSIS_HOOK_PREINSTALL
  nsExec::Exec 'taskkill /F /IM havenkeys-native-host.exe'
  Pop $0
!macroend
```

In `apps/desktop/src-tauri/tauri.conf.json`, inside `"bundle"`, add:

```json
    "windows": {
      "nsis": {
        "installerHooks": "./windows/hooks.nsh"
      }
    },
```

- [ ] **Step 3: Sign in the workflow**

In `.github/workflows/release.yml`, step "Build and attach installers":

```yaml
      - name: Build and attach installers
        uses: tauri-apps/tauri-action@84b9d35b5fc46c1e45415bdb6144030364f7ebc5 # v0.6.2
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          # Signs the update artifacts (`.sig`) that the app verifies before
          # installing. The public half is in tauri.conf.json.
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
        with:
          projectPath: apps/desktop
          tagName: ${{ github.ref_name }}
          releaseName: "HavenKeys ${{ github.ref_name }}"
          # Also the "What's new" text in the app (latest.json `notes`).
          releaseBody: "Installers for this release. Unsigned: Windows SmartScreen and macOS Gatekeeper will warn on first run."
          releaseDraft: true
          prerelease: false
          # One latest.json for all three platforms; Windows updates through
          # the NSIS installer, not the MSI.
          includeUpdaterJson: true
          updaterJsonPreferNsis: true
          args: --config src-tauri/tauri.bundle.conf.json ${{ matrix.args }}
```

(Only the `env` additions, the comment on `releaseBody`, and the two `includeUpdaterJson`/`updaterJsonPreferNsis` lines are new.)

- [ ] **Step 4: Check the config locally**

Run: `node -e "JSON.parse(require('fs').readFileSync('apps/desktop/src-tauri/tauri.conf.json','utf8')); JSON.parse(require('fs').readFileSync('apps/desktop/src-tauri/tauri.bundle.conf.json','utf8')); console.log('ok')"`
Expected: `ok`.

Run: `cargo build -p havenkeys-desktop`
Expected: builds (tauri-build validates `tauri.conf.json`, including the `plugins.updater` and `bundle.windows.nsis` shapes).

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/tauri.bundle.conf.json apps/desktop/src-tauri/tauri.conf.json apps/desktop/src-tauri/windows/hooks.nsh .github/workflows/release.yml
git commit -m "ci: sign update artifacts and publish latest.json; free the native host before installing"
```

---

### Task 7: Documentation, audits and the security review entry

**Files:**
- Modify: `CLAUDE.md` (§1 amendment note)
- Modify: `docs/security-model.md`, `docs/threat-model.md`, `docs/development.md`, `docs/website.md`, `README.md`, `docs/security-review.md`
- Modify: `THIRD-PARTY-NOTICES.md` if new crates need listing (follow how existing crates are listed there)

- [ ] **Step 1: CLAUDE.md amendment**

In `CLAUDE.md` §1, directly after the existing "Amended on 2026-09-20" quote block, add:

```markdown
> Amended on 2026-09-27 by
> `docs/superpowers/specs/2026-09-27-desktop-auto-update-design.md`: the
> desktop app may contact GitHub Releases to check for and download signed
> updates. Nothing is installed without the user's click; the automatic
> check can be turned off in Settings → Updates.
```

- [ ] **Step 2: Security and threat model**

In `docs/security-model.md`, add a section "In-app updates" covering, in prose: the only new destinations (`github.com`, and GitHub's download redirect host) over HTTPS; what the request reveals (IP address, time, that HavenKeys is checking; no account, device ID or vault data); integrity from the minisign signature (public key in `tauri.conf.json`, key ID `1D3AD839EC826779`), not from TLS; no downgrade (the plugin installs only a greater version); order download → verify → lock → install; the renderer has no updater plugin permission, only the four commands; notes are plain text and truncated to 4 000 characters; the "Download" URL is a constant; the setting lives in `updates.json` (not secret, outside the vault); debug builds never check automatically. Also update any "no network except the user's server" statement to mention this exception.

In `docs/threat-model.md`, add threats with mitigations and residual risk:
- Malicious update via a compromised GitHub account/repository/CDN or network → rejected without the private key.
- Theft of the updater private key and passphrase → can ship code to every install, including the Rust core; residual trust assumption; mitigations: key only in repository secrets with a passphrase, offline backup, drafts reviewed before publishing, rotation procedure.
- Replay of an old signed release → refused by version comparison.
- Loss of the private key → users must install the next version by hand.
- Privacy: GitHub sees update checks; can be turned off.

- [ ] **Step 3: Development and release docs**

In `docs/development.md`, add "Releases and updates":
- The key was generated once by the owner (`pnpm tauri signer generate -w ~/.tauri/havenkeys-updater.key`); private key and passphrase live only in repository secrets `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` and an offline backup; never commit or paste them.
- `createUpdaterArtifacts` is set only in `tauri.bundle.conf.json` (CI), so local `tauri build` does not need the key.
- Releasing: bump the version, push a `desktop-v*` tag, wait for the draft, check that `latest.json` has `windows-x86_64`, `darwin-aarch64`, `darwin-x86_64`, `linux-x86_64`, then publish — publishing is what makes installs see it.
- The in-app "What's new" comes from `releaseBody` at build time; editing the release page later does not change it (edit the `latest.json` asset before publishing if needed).
- Key rotation: ship one release signed with the old key whose `tauri.conf.json` has the new public key; sign every later release with the new key.

In `docs/website.md` and `README.md`: from 0.9.0 the app updates itself on Windows, macOS and the AppImage; `.deb`/`.rpm` users download new versions (the app tells them); 0.8.0 and earlier must install 0.9.0 by hand, over the existing install (no uninstall needed).

- [ ] **Step 4: Audits**

Run: `pnpm audit && cargo audit && cargo deny check`
Expected: no new advisories or licence failures. If `cargo deny check licenses` rejects a licence brought in by `tauri-plugin-updater`'s tree, inspect it (`cargo tree -p havenkeys-desktop -i <crate>`), and only if it is a permissive OSI licence add it to `deny.toml`'s allow list with a comment naming the crate; otherwise stop and report.

If `THIRD-PARTY-NOTICES.md` enumerates crates, add the new ones in the same format (`cargo tree -p havenkeys-desktop -e normal --prefix none | sort -u` diffed against before the change shows what is new).

- [ ] **Step 5: Security review entry**

Append to `docs/security-review.md` a finding in its existing format: component "Desktop updater"; attack scenarios (malicious update, key theft, downgrade, notes injection, running-host file lock); mitigations as implemented; severity of the residual key-theft risk; remaining limitations (installers not code-signed, so the first manual install is still unverified by the OS; `.deb`/`.rpm` not updated in place; GitHub sees checks).

- [ ] **Step 6: Commit**

```bash
git add CLAUDE.md docs README.md THIRD-PARTY-NOTICES.md deny.toml
git commit -m "docs: in-app updates — security model, threats, release steps"
```

---

### Task 8: End-to-end release test (owner-assisted)

Needs pushing tags and publishing releases, and installing on real Windows/macOS/Linux machines. **Ask the owner before pushing any tag or publishing any release.**

- [ ] **Step 1: First updater-enabled release**

With the owner's go-ahead: bump the version to `0.9.0` the way earlier bumps did (`git show 0ce9c0f --stat` lists the files), commit `chore: bump version to 0.9.0`, push `main` and tag `desktop-v0.9.0`. When the workflow finishes, inspect the draft:

Run: `gh release view desktop-v0.9.0 --json assets -q '.assets[].name' | grep -E 'latest.json|\.sig$'`
Expected: `latest.json` and a `.sig` per updatable artifact.

Run: `gh release download desktop-v0.9.0 -p latest.json -O - | node -e "const j=JSON.parse(require('fs').readFileSync(0,'utf8'));console.log(j.version,Object.keys(j.platforms).sort().join(' '))"`
Expected: `0.9.0` and keys including `darwin-aarch64 darwin-x86_64 linux-x86_64 windows-x86_64`, with the Windows URL ending in `-setup.exe`.

The owner publishes the draft and installs 0.9.0 by hand over 0.8.0 on each OS (no uninstall).

- [ ] **Step 2: Second release, updated in-app**

Bump to `0.9.1`, tag, wait, check `latest.json` as above, owner publishes. On each OS, with 0.9.0 running (vault unlocked, a browser with the extension open):
- within a few seconds of launch (or via Settings → Check now) the banner shows "HavenKeys 0.9.1 is available", the tray has "Update available";
- "What's new" shows the release body as text;
- Update → progress → the vault locks → the app restarts on 0.9.1 (Settings → About), on the unlock screen;
- Windows: the install completes although the browser held the native host; the extension works again after unlocking (fill a login);
- a `.deb` install shows "Download", which opens the release page;
- turning the switch off, quitting and relaunching: no check (no banner) until "Check now".

- [ ] **Step 3: Tampered update is refused**

On one machine, point a local build at a tampered manifest: build with `plugins.updater.endpoints` set to a local file server (`python3 -m http.server` serving a copy of `latest.json` whose `linux-x86_64.signature` is replaced by another release's signature). Run the AppImage built from that config, Check now → Update. Expected: "The update failed. Try again later.", the vault stays unlocked, the running version is unchanged. Discard that build and config change afterwards (`git diff` clean).

- [ ] **Step 4: Record results**

Add the outcome of Steps 1–3 (per OS: pass/fail, notes) to `docs/security-review.md` under the updater finding, and commit:

```bash
git add docs/security-review.md
git commit -m "docs: record the end-to-end updater test"
```
