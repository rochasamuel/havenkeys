//! Auto-lock policy.
//!
//! Pure logic: callers feed it a monotonic timestamp (time since some fixed
//! point, not counting suspend on Linux/macOS) and a wall-clock timestamp.
//! It never sleeps or spawns threads, which keeps it deterministic to test.

use std::time::Duration;

/// If the wall clock advances this much more than the monotonic clock between
/// two ticks, assume the machine was suspended.
pub const SUSPEND_THRESHOLD: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockReason {
    Idle,
    Suspend,
}

impl LockReason {
    pub fn as_str(self) -> &'static str {
        match self {
            LockReason::Idle => "idle",
            LockReason::Suspend => "suspend",
        }
    }
}

#[derive(Debug, Default)]
pub struct LockManager {
    timeout: Option<Duration>,
    last_activity: Duration,
    last_tick: Option<(Duration, Duration)>,
}

impl LockManager {
    pub fn new(timeout: Option<Duration>) -> Self {
        Self {
            timeout,
            ..Default::default()
        }
    }

    /// Timeout from the `auto_lock_minutes` setting (0 = never).
    pub fn timeout_from_minutes(minutes: u32) -> Option<Duration> {
        (minutes > 0).then(|| Duration::from_secs(u64::from(minutes) * 60))
    }

    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }

    /// Call when the vault is unlocked.
    pub fn reset(&mut self, mono: Duration, wall: Duration) {
        self.last_activity = mono;
        self.last_tick = Some((mono, wall));
    }

    pub fn record_activity(&mut self, mono: Duration) {
        if mono > self.last_activity {
            self.last_activity = mono;
        }
    }

    /// Returns a reason if the vault should be locked now.
    pub fn tick(&mut self, mono: Duration, wall: Duration) -> Option<LockReason> {
        let previous = self.last_tick.replace((mono, wall));
        if let Some((prev_mono, prev_wall)) = previous {
            let mono_delta = mono.saturating_sub(prev_mono);
            let wall_delta = wall.saturating_sub(prev_wall);
            if wall_delta > mono_delta + SUSPEND_THRESHOLD {
                return Some(LockReason::Suspend);
            }
        }
        match self.timeout {
            Some(t) if mono.saturating_sub(self.last_activity) >= t => Some(LockReason::Idle),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn s(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    #[test]
    fn idle_timeout() {
        let mut m = LockManager::new(Some(s(300)));
        m.reset(s(0), s(1000));
        assert_eq!(m.tick(s(299), s(1299)), None);
        assert_eq!(m.tick(s(300), s(1300)), Some(LockReason::Idle));
    }

    #[test]
    fn activity_postpones_lock() {
        let mut m = LockManager::new(Some(s(300)));
        m.reset(s(0), s(0));
        m.record_activity(s(200));
        assert_eq!(m.tick(s(450), s(450)), None);
        assert_eq!(m.tick(s(500), s(500)), Some(LockReason::Idle));
        // Out-of-order activity timestamps never move the timer backwards.
        m.record_activity(s(10));
        assert_eq!(m.tick(s(505), s(505)), Some(LockReason::Idle));
    }

    #[test]
    fn never_means_never() {
        let mut m = LockManager::new(LockManager::timeout_from_minutes(0));
        m.reset(s(0), s(0));
        assert_eq!(m.tick(s(1_000_000), s(1_000_000)), None);
    }

    #[test]
    fn suspend_detected_even_with_never() {
        let mut m = LockManager::new(None);
        m.reset(s(0), s(0));
        assert_eq!(m.tick(s(5), s(5)), None);
        // 5 s monotonic, 1 h wall: machine slept.
        assert_eq!(m.tick(s(10), s(3605)), Some(LockReason::Suspend));
    }

    #[test]
    fn small_clock_skew_is_tolerated() {
        let mut m = LockManager::new(None);
        m.reset(s(0), s(0));
        assert_eq!(m.tick(s(5), s(20)), None);
        // Wall clock moving backwards (NTP) does not lock.
        assert_eq!(m.tick(s(10), s(1)), None);
    }

    #[test]
    fn minutes_conversion() {
        assert_eq!(LockManager::timeout_from_minutes(15), Some(s(900)));
        assert_eq!(LockManager::timeout_from_minutes(0), None);
    }
}
