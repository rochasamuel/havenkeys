//! Token-bucket rate limiting for bridge requests.
//!
//! Every legitimate request follows a user action, so the limits are generous
//! for people and tight for a script that tries to walk the vault. The limiter
//! is shared by all connections, so opening new connections does not reset it.

use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestClass {
    /// `find_matches`: no secrets, but reveals which sites have logins.
    Lookup,
    /// `fill_item`, `get_totp`: return a secret.
    Secret,
}

struct Bucket {
    capacity: f64,
    per_second: f64,
    tokens: f64,
    last: Option<Instant>,
}

impl Bucket {
    const fn new(capacity: f64, per_second: f64) -> Self {
        Self {
            capacity,
            per_second,
            tokens: capacity,
            last: None,
        }
    }

    fn take(&mut self, now: Instant) -> bool {
        if let Some(last) = self.last {
            let elapsed = now.saturating_duration_since(last).as_secs_f64();
            self.tokens = (self.tokens + elapsed * self.per_second).min(self.capacity);
        }
        self.last = Some(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

pub struct RateLimiter {
    lookup: Bucket,
    secret: Bucket,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self {
            // Burst of 60, then 5 per second.
            lookup: Bucket::new(60.0, 5.0),
            // Burst of 10, then one every 2 seconds (30 per minute).
            secret: Bucket::new(10.0, 0.5),
        }
    }
}

impl RateLimiter {
    /// Consume one token of `class`. False means the request must be refused.
    pub fn allow(&mut self, class: RequestClass, now: Instant) -> bool {
        match class {
            RequestClass::Lookup => self.lookup.take(now),
            RequestClass::Secret => self.secret.take(now),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn burst_then_refill() {
        let mut rl = RateLimiter::default();
        let t0 = Instant::now();
        for _ in 0..10 {
            assert!(rl.allow(RequestClass::Secret, t0));
        }
        assert!(!rl.allow(RequestClass::Secret, t0));
        // Lookups have their own bucket.
        assert!(rl.allow(RequestClass::Lookup, t0));
        assert!(!rl.allow(RequestClass::Secret, t0 + Duration::from_millis(1500)));
        assert!(rl.allow(RequestClass::Secret, t0 + Duration::from_secs(2)));
    }

    #[test]
    fn refill_is_capped() {
        let mut rl = RateLimiter::default();
        let t0 = Instant::now();
        assert!(rl.allow(RequestClass::Secret, t0));
        let later = t0 + Duration::from_secs(3600);
        for _ in 0..10 {
            assert!(rl.allow(RequestClass::Secret, later));
        }
        assert!(!rl.allow(RequestClass::Secret, later));
    }
}
