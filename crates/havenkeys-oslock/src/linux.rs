//! logind `LockedHint`, read with `loginctl`.
//!
//! Spawning a fixed program with fixed arguments every few seconds is cheap,
//! and it avoids linking a D-Bus stack. Nothing user-controlled reaches the
//! command line: the session ID comes from `XDG_SESSION_ID` only if it is
//! plain alphanumeric, otherwise logind's own "auto" is used.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CANDIDATES: [&str; 2] = ["/usr/bin/loginctl", "/bin/loginctl"];
const TIMEOUT: Duration = Duration::from_secs(2);
/// Consecutive failures before giving up for the rest of the run.
const MAX_FAILURES: u32 = 3;

pub struct Probe {
    program: Option<&'static str>,
    session: String,
    failures: u32,
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
            failures: 0,
        }
    }

    pub fn locked(&mut self) -> Option<bool> {
        let program = self.program?;
        let result = self.query(program);
        if result.is_none() {
            self.failures += 1;
            if self.failures >= MAX_FAILURES {
                self.program = None;
            }
        } else {
            self.failures = 0;
        }
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
        let deadline = Instant::now() + TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => break,
                Ok(Some(_)) | Err(_) => return None,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            }
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

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_loginctl_values() {
        assert_eq!(parse("yes\n"), Some(true));
        assert_eq!(parse("no\n"), Some(false));
        for junk in ["", "maybe", "Yes", "yes no", "\u{0}"] {
            assert_eq!(parse(junk), None, "{junk:?}");
        }
    }
}
