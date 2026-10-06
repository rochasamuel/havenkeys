//! logind `LockedHint`, read with `loginctl`.
//!
//! Spawning a fixed program with fixed arguments every few seconds is cheap,
//! and it avoids linking a D-Bus stack. Nothing user-controlled reaches the
//! command line: the session ID comes from `XDG_SESSION_ID` only if it is
//! plain alphanumeric, otherwise logind's own "auto" is used.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const CANDIDATES: [&str; 2] = ["/usr/bin/loginctl", "/bin/loginctl"];
const TIMEOUT: Duration = Duration::from_secs(2);
/// Consecutive failures before the probe pauses (or, if it never worked,
/// gives up for the rest of the run: no logind here).
const MAX_FAILURES: u32 = 3;
/// The first pause after failures, doubled each time, up to `MAX_PAUSE`.
const FIRST_PAUSE: Duration = Duration::from_secs(30);
const MAX_PAUSE: Duration = Duration::from_secs(5 * 60);

pub struct Probe {
    program: Option<&'static str>,
    session: String,
    health: Health,
}

/// When to ask `loginctl` again. A probe that has answered before only
/// pauses after failures (a load spike, a logind restart, a resume), so
/// screen-lock locking comes back by itself (DT4); one that never answered
/// stops, as on systems without logind.
#[derive(Debug, Default)]
struct Health {
    worked: bool,
    failures: u32,
    pause: Option<Duration>,
    paused_until: Option<Instant>,
    off: bool,
}

impl Health {
    fn may_ask(&self, now: Instant) -> bool {
        !self.off && self.paused_until.is_none_or(|t| now >= t)
    }

    fn record(&mut self, answered: bool, now: Instant) {
        if answered {
            *self = Self {
                worked: true,
                ..Self::default()
            };
            return;
        }
        self.failures += 1;
        if self.failures < MAX_FAILURES {
            return;
        }
        if !self.worked {
            self.off = true;
            return;
        }
        let pause = self.pause.map_or(FIRST_PAUSE, |p| (p * 2).min(MAX_PAUSE));
        self.pause = Some(pause);
        self.paused_until = Some(now + pause);
        self.failures = 0;
    }
}

fn session_id() -> String {
    std::env::var("XDG_SESSION_ID")
        .ok()
        .filter(|s| !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_alphanumeric()))
        .unwrap_or_else(|| "auto".to_owned())
}

/// `yes` / `no` from `loginctl … --value`.
pub(crate) fn parse(output: &str) -> Option<bool> {
    match output.trim() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

impl Probe {
    pub fn new() -> Self {
        Self {
            program: CANDIDATES
                .into_iter()
                .find(|p| std::path::Path::new(p).is_file()),
            session: session_id(),
            health: Health::default(),
        }
    }

    pub fn locked(&mut self) -> Option<bool> {
        let program = self.program?;
        let now = Instant::now();
        if !self.health.may_ask(now) {
            return None;
        }
        let result = self.query(program);
        self.health.record(result.is_some(), now);
        result
    }

    fn query(&self, program: &str) -> Option<bool> {
        let mut child = Command::new(program)
            .args([
                "show-session",
                &self.session,
                "--property=LockedHint",
                "--value",
            ])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        if !exits_successfully_within(&mut child, TIMEOUT) {
            return None;
        }
        let mut out = String::new();
        child
            .stdout
            .take()?
            .take(64)
            .read_to_string(&mut out)
            .ok()?;
        parse(&out)
    }
}

/// Wait for `child` to exit successfully. A child still running at the
/// deadline is killed and reaped.
fn exits_successfully_within(child: &mut Child, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Err(_) => return false,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{exits_successfully_within, parse, Health, FIRST_PAUSE, MAX_PAUSE};
    use std::process::Command;
    use std::time::{Duration, Instant};

    #[test]
    fn waits_for_success_failure_and_the_deadline() {
        let ok = |program: &str, args: &[&str], timeout| {
            let mut child = Command::new(program).args(args).spawn().unwrap();
            exits_successfully_within(&mut child, timeout)
        };
        assert!(ok("true", &[], Duration::from_secs(2)));
        assert!(!ok("false", &[], Duration::from_secs(2)));

        let started = Instant::now();
        let mut child = Command::new("sleep").arg("5").spawn().unwrap();
        assert!(!exits_successfully_within(
            &mut child,
            Duration::from_millis(100)
        ));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "stopped at the deadline"
        );
        assert!(child.try_wait().unwrap().is_some(), "the child was reaped");
    }

    /// DT4: failures after the probe has worked only pause it.
    #[test]
    fn failures_pause_a_probe_that_worked_and_it_comes_back() {
        let t0 = Instant::now();
        let mut h = Health::default();
        h.record(true, t0);
        for _ in 0..3 {
            assert!(h.may_ask(t0));
            h.record(false, t0);
        }
        assert!(!h.may_ask(t0));
        assert!(!h.may_ask(t0 + FIRST_PAUSE - Duration::from_secs(1)));
        assert!(h.may_ask(t0 + FIRST_PAUSE));

        // Pauses grow, up to the cap.
        let mut t = t0 + FIRST_PAUSE;
        for _ in 0..10 {
            for _ in 0..3 {
                h.record(false, t);
            }
            t += MAX_PAUSE;
            assert!(h.may_ask(t));
        }
        assert_eq!(h.pause, Some(MAX_PAUSE));

        h.record(true, t);
        assert!(h.may_ask(t));
        assert_eq!(h.pause, None);
    }

    #[test]
    fn a_probe_that_never_worked_switches_off() {
        let t0 = Instant::now();
        let mut h = Health::default();
        for _ in 0..3 {
            h.record(false, t0);
        }
        assert!(!h.may_ask(t0 + Duration::from_secs(3600)));
    }

    #[test]
    fn parses_loginctl_values() {
        assert_eq!(parse("yes\n"), Some(true));
        assert_eq!(parse("no\n"), Some(false));
        for junk in ["", "maybe", "Yes", "yes no", "\u{0}"] {
            assert_eq!(parse(junk), None, "{junk:?}");
        }
    }
}
