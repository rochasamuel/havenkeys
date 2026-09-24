//! Token-bucket rate limiting for bridge requests.
//!
//! Every legitimate request follows a user action, so the limits are generous
//! for people and tight for a script that tries to walk the vault. The limiter
//! is shared by all connections, so opening new connections does not reset it.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestClass {
    /// `find_matches`, `find_passkeys`, `check_passkey_create`,
    /// `passkey_status`: no secrets, but reveals which sites have logins or
    /// passkeys.
    Lookup,
    /// `fill_item`, `get_totp`, `passkey_get`, `passkey_create`: return a
    /// secret (a password, TOTP code, WebAuthn assertion, or a new
    /// passkey's registration).
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
    /// Last browser-initiated password change per item.
    updates: HashMap<Uuid, Instant>,
}

/// One password change per item from the browser in this window. A caller
/// that floods updates cannot push the real password out of the item's
/// password history (5 entries) in less than this × 5.
pub const ITEM_UPDATE_INTERVAL: Duration = Duration::from_secs(10 * 60);

impl Default for RateLimiter {
    fn default() -> Self {
        Self {
            // Burst of 60, then 5 per second.
            lookup: Bucket::new(60.0, 5.0),
            // Burst of 10, then one every 2 seconds (30 per minute).
            secret: Bucket::new(10.0, 0.5),
            updates: HashMap::new(),
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

    /// May the browser change `item`'s password now? Records the change if so.
    pub fn allow_item_update(&mut self, item: Uuid, now: Instant) -> bool {
        self.updates
            .retain(|_, t| now.saturating_duration_since(*t) < ITEM_UPDATE_INTERVAL);
        if self.updates.contains_key(&item) {
            return false;
        }
        self.updates.insert(item, now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn item_updates_have_a_cooldown() {
        let mut rl = RateLimiter::default();
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let t0 = Instant::now();
        assert!(rl.allow_item_update(a, t0));
        assert!(!rl.allow_item_update(a, t0 + Duration::from_secs(60)));
        assert!(rl.allow_item_update(b, t0), "per item");
        assert!(rl.allow_item_update(a, t0 + ITEM_UPDATE_INTERVAL));
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
