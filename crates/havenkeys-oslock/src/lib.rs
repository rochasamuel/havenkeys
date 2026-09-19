//! Is the operating-system session locked?
//!
//! The desktop app polls this from its auto-lock thread and locks the vault
//! when the screen locks (docs/security-model.md §5).
//!
//! * **Windows:** `WTSQuerySessionInformationW(WTSSessionInfoEx)`, the
//!   session's lock flag (Windows 8 and later). This is the only `unsafe`
//!   code in HavenKeys, kept in `windows.rs`.
//! * **Linux:** logind's `LockedHint` for this session, read with
//!   `loginctl`. GNOME, KDE and other logind-aware lockers set it. Without
//!   systemd-logind (some minimal setups, WSL) the probe reports "unknown"
//!   and switches itself off.
//! * **Other platforms:** always unknown.
//!
//! "Unknown" never locks the vault; idle and suspend locking still apply.

#![deny(unsafe_code)]
#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

/// Turns a stream of lock-state observations into "lock now" events.
///
/// Fires when the session is seen locked after it was last seen unlocked or
/// unknown, and only once per lock. Firing on each observation instead would
/// make the vault unusable on a desktop whose lock flag gets stuck.
#[derive(Debug, Default)]
pub struct EdgeDetector {
    last: Option<bool>,
}

impl EdgeDetector {
    /// Feed one observation (`None` = unknown). True means "lock now".
    pub fn observe(&mut self, locked: Option<bool>) -> bool {
        let fire = locked == Some(true) && self.last != Some(true);
        if locked.is_some() {
            self.last = locked;
        }
        fire
    }
}

/// Polls the platform's session lock state.
pub struct SessionWatcher {
    probe: Probe,
    edge: EdgeDetector,
}

impl Default for SessionWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionWatcher {
    pub fn new() -> Self {
        Self {
            probe: Probe::new(),
            edge: EdgeDetector::default(),
        }
    }

    /// Check once. True when the session has just locked. Blocks for at
    /// most a couple of seconds; call it from a background thread.
    pub fn poll(&mut self) -> bool {
        let state = self.probe.locked();
        self.edge.observe(state)
    }
}

#[cfg(target_os = "linux")]
use linux::Probe;
#[cfg(windows)]
use windows::Probe;

#[cfg(not(any(target_os = "linux", windows)))]
struct Probe;

#[cfg(not(any(target_os = "linux", windows)))]
impl Probe {
    fn new() -> Self {
        Probe
    }

    fn locked(&mut self) -> Option<bool> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::EdgeDetector;

    #[test]
    fn fires_once_per_lock() {
        let mut e = EdgeDetector::default();
        assert!(!e.observe(Some(false)));
        assert!(e.observe(Some(true)));
        assert!(!e.observe(Some(true)), "still locked: no second event");
        assert!(!e.observe(Some(false)));
        assert!(e.observe(Some(true)), "locked again");
    }

    #[test]
    fn unknown_neither_fires_nor_forgets() {
        let mut e = EdgeDetector::default();
        assert!(!e.observe(None));
        assert!(e.observe(Some(true)), "first sight of a lock fires");
        assert!(!e.observe(None));
        assert!(
            !e.observe(Some(true)),
            "a gap in knowledge is not a new lock"
        );
    }

    #[test]
    fn stuck_lock_flag_fires_only_once() {
        let mut e = EdgeDetector::default();
        let fired = (0..100).filter(|_| e.observe(Some(true))).count();
        assert_eq!(fired, 1);
    }

    #[test]
    fn watcher_never_panics_here() {
        // In CI/WSL there is usually no logind session: must just say "no".
        let mut w = super::SessionWatcher::new();
        for _ in 0..3 {
            let _ = w.poll();
        }
    }
}
