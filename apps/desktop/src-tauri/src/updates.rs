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
        std::fs::read(dir.join(FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Written to a temporary file and renamed over the old one.
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
    /// The phase a running check replaced, so a background failure can put
    /// back an offer it would otherwise hide.
    before_check: Phase,
}

impl Machine {
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            before_check: Phase::Idle,
        }
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    /// Starts a check unless one is running or an update is being
    /// downloaded or installed. `false` means "do nothing".
    pub fn begin_check(&mut self) -> bool {
        match self.phase {
            Phase::Checking | Phase::Downloading { .. } | Phase::Installing => false,
            _ => {
                self.before_check = std::mem::replace(&mut self.phase, Phase::Checking);
                true
            }
        }
    }

    /// A background check that fails is not worth interrupting anyone for;
    /// only a check the user asked for reports failure. A background failure
    /// keeps an update already on offer (the found `Update` is kept too).
    pub fn finish_check(&mut self, outcome: CheckOutcome, manual: bool) {
        let before = std::mem::replace(&mut self.before_check, Phase::Idle);
        self.phase = match outcome {
            CheckOutcome::Found { version, notes } => Phase::Available {
                version,
                notes: truncate_notes(&notes),
            },
            CheckOutcome::UpToDate => Phase::Idle,
            CheckOutcome::Failed if manual => Phase::Failed {
                during: Stage::Check,
            },
            CheckOutcome::Failed => match before {
                offer @ Phase::Available { .. } => offer,
                _ => Phase::Idle,
            },
        };
    }

    /// Only from `Available`, so a second click cannot start a second
    /// download. Returns the version being installed.
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

    /// Adds a downloaded chunk. `true` when the change is worth telling the
    /// UI about: the whole percentage moved, or (size unknown) another MiB.
    pub fn progress(&mut self, chunk: u64, total: Option<u64>) -> bool {
        let Phase::Downloading {
            downloaded,
            total: known,
            ..
        } = &mut self.phase
        else {
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
    match notes.char_indices().nth(NOTES_LIMIT) {
        None => notes.to_owned(),
        Some((cut, _)) => format!("{}…", &notes[..cut]),
    }
}

/// Whether this install can replace itself. The updater handles the
/// Windows installer, the macOS app bundle and the AppImage; a `.deb` or
/// `.rpm` install belongs to the package manager.
pub fn can_install_in_place(os: &str, appimage: bool) -> bool {
    match os {
        "windows" | "macos" => true,
        "linux" => appimage,
        _ => false,
    }
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
        assert_eq!(
            UpdateSettings::load(dir.path()),
            UpdateSettings { auto_check: true }
        );
    }

    #[test]
    fn settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        UpdateSettings { auto_check: false }
            .save(dir.path())
            .unwrap();
        assert_eq!(
            UpdateSettings::load(dir.path()),
            UpdateSettings { auto_check: false }
        );
        UpdateSettings { auto_check: true }
            .save(dir.path())
            .unwrap();
        assert_eq!(
            UpdateSettings::load(dir.path()),
            UpdateSettings { auto_check: true }
        );
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
        assert_eq!(
            UpdateSettings::load(dir.path()),
            UpdateSettings { auto_check: false }
        );
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
        let mode = std::fs::metadata(dir.path().join(FILE))
            .unwrap()
            .permissions()
            .mode();
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
            &Phase::Available {
                version: "0.9.0".into(),
                notes: "Fixes.".into()
            }
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
        assert_eq!(
            m.phase(),
            &Phase::Failed {
                during: Stage::Check
            }
        );
        // A failure does not block the next check.
        assert!(m.begin_check());
    }

    #[test]
    fn a_failed_background_check_keeps_the_offer() {
        let mut m = Machine::new();
        m.begin_check();
        m.finish_check(found("0.9.0"), false);
        assert!(m.begin_check());
        m.finish_check(CheckOutcome::Failed, false);
        assert_eq!(
            m.phase(),
            &Phase::Available {
                version: "0.9.0".into(),
                notes: "Fixes.".into()
            }
        );
        // The offer can still be installed.
        assert_eq!(m.begin_install(), Ok("0.9.0".into()));
    }

    #[test]
    fn a_failed_manual_check_is_reported_even_with_an_offer() {
        let mut m = Machine::new();
        m.begin_check();
        m.finish_check(found("0.9.0"), false);
        m.begin_check();
        m.finish_check(CheckOutcome::Failed, true);
        assert_eq!(
            m.phase(),
            &Phase::Failed {
                during: Stage::Check
            }
        );
    }

    #[test]
    fn a_failed_background_check_after_a_failure_goes_idle() {
        let mut m = Machine::new();
        m.begin_check();
        m.finish_check(CheckOutcome::Failed, true);
        m.begin_check();
        m.finish_check(CheckOutcome::Failed, false);
        assert_eq!(m.phase(), &Phase::Idle);
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
            &Phase::Downloading {
                version: "0.9.0".into(),
                downloaded: 0,
                total: None
            }
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
        assert_eq!(
            m.phase(),
            &Phase::Failed {
                during: Stage::Install
            }
        );
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
            &Phase::Downloading {
                version: "0.9.0".into(),
                downloaded: 1000,
                total: Some(1000)
            }
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
            CheckOutcome::Found {
                version: "0.9.0".into(),
                notes: "x".repeat(1_000_000),
            },
            false,
        );
        let Phase::Available { notes, .. } = m.phase() else {
            panic!()
        };
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
            json(Phase::Available {
                version: "0.9.0".into(),
                notes: "n".into()
            }),
            serde_json::json!({ "phase": "available", "version": "0.9.0", "notes": "n" })
        );
        assert_eq!(
            json(Phase::Downloading {
                version: "0.9.0".into(),
                downloaded: 5,
                total: None
            }),
            serde_json::json!({ "phase": "downloading", "version": "0.9.0", "downloaded": 5, "total": null })
        );
        assert_eq!(
            json(Phase::Failed {
                during: Stage::Install
            }),
            serde_json::json!({ "phase": "failed", "during": "install" })
        );
    }
}
