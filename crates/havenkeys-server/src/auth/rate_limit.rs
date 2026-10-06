//! Login rate limiting.
//!
//! Counters are kept per account and per source address, so one noisy network
//! cannot lock out an account it does not own, and one account cannot be
//! ground down from many addresses. The key for an unknown email is the decoy
//! account id (`routes::auth`), which means probing a non-existent account is
//! rate limited exactly like probing a real one.
//!
//! A check of the auth key is charged before it runs (`AttemptKeys::charge`)
//! and the charge is cleared when the key is right. Checking and charging
//! happen under one row lock, so a burst of parallel attempts cannot all pass
//! the check before any of them is counted (SV-4).

use crate::error::ApiError;
use deadpool_postgres::Object;
use uuid::Uuid;

const MAX_FAILURES: i32 = 5;
const WINDOW_MINUTES: i32 = 15;

/// Escalating block once the window's failures pass the threshold.
pub fn block_seconds(failures: i32) -> i64 {
    match failures {
        ..=5 => 60,
        6..=9 => 5 * 60,
        _ => 30 * 60,
    }
}

/// Refuse the attempt outright while a block is in force.
pub async fn check(db: &Object, key: &str) -> Result<(), ApiError> {
    let blocked = db
        .query_opt(
            "SELECT 1 FROM login_attempts WHERE key = $1 AND blocked_until > now()",
            &[&key],
        )
        .await?;
    match blocked {
        Some(_) => Err(ApiError::RateLimited),
        None => Ok(()),
    }
}

/// Count one failure, starting a fresh window if the last one has elapsed,
/// and arm the block once the threshold is reached.
pub async fn record_failure(db: &Object, key: &str) -> Result<(), ApiError> {
    let row = db
        .query_one(
            "INSERT INTO login_attempts (key, failures, window_start)
             VALUES ($1, 1, now())
             ON CONFLICT (key) DO UPDATE SET
               failures = CASE
                 WHEN login_attempts.window_start < now() - make_interval(mins => $2)
                 THEN 1 ELSE login_attempts.failures + 1 END,
               window_start = CASE
                 WHEN login_attempts.window_start < now() - make_interval(mins => $2)
                 THEN now() ELSE login_attempts.window_start END,
               blocked_until = CASE
                 WHEN login_attempts.window_start < now() - make_interval(mins => $2)
                 THEN NULL ELSE login_attempts.blocked_until END
             RETURNING failures",
            &[&key, &WINDOW_MINUTES],
        )
        .await?;
    let failures: i32 = row.get(0);
    if failures >= MAX_FAILURES {
        db.execute(
            "UPDATE login_attempts
                SET blocked_until = now() + make_interval(secs => $2)
              WHERE key = $1",
            &[&key, &(block_seconds(failures) as f64)],
        )
        .await?;
    }
    Ok(())
}

/// A successful login clears the counter for that key.
pub async fn clear(db: &Object, key: &str) -> Result<(), ApiError> {
    db.execute("DELETE FROM login_attempts WHERE key = $1", &[&key])
        .await?;
    Ok(())
}

/// The counter for one source address. An IPv6 address counts as its /64:
/// one host is routinely given a whole /64, and could otherwise spread its
/// attempts over as many addresses as it likes (SV-4).
pub fn ip_key(ip: &str) -> String {
    let Ok(std::net::IpAddr::V6(v6)) = ip.parse::<std::net::IpAddr>() else {
        return format!("ip:{ip}");
    };
    if let Some(v4) = v6.to_ipv4_mapped() {
        return format!("ip:{v4}");
    }
    let s = v6.segments();
    format!("ip:{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
}

/// The two counters a check of the auth key spends, the account's and the
/// caller's address's: checked, charged and cleared together, account first.
pub struct AttemptKeys {
    account: String,
    ip: String,
}

impl AttemptKeys {
    pub fn new(account_id: Uuid, ip: &str) -> Self {
        Self {
            account: format!("acct:{account_id}"),
            ip: ip_key(ip),
        }
    }

    /// Refuse the attempt while either counter is blocked; otherwise count
    /// it as a failure now, arming the block once the threshold is reached.
    /// One transaction, both rows locked (account first, so two attempts
    /// never wait on each other in opposite order), so parallel attempts are
    /// counted one after another. A right key then calls `clear`.
    pub async fn charge(&self, db: &mut Object) -> Result<(), ApiError> {
        let tx = db.transaction().await?;
        let keys = [&self.account, &self.ip];
        for key in keys {
            tx.execute(
                "INSERT INTO login_attempts (key, failures, window_start)
                 VALUES ($1, 0, now()) ON CONFLICT (key) DO NOTHING",
                &[key],
            )
            .await?;
        }
        for key in keys {
            let blocked: bool = tx
                .query_one(
                    "SELECT coalesce(blocked_until > now(), false)
                       FROM login_attempts WHERE key = $1 FOR UPDATE",
                    &[key],
                )
                .await?
                .get(0);
            if blocked {
                // Rolled back: nothing is counted for a refused attempt.
                return Err(ApiError::RateLimited);
            }
        }
        for key in keys {
            let failures: i32 = tx
                .query_one(
                    "UPDATE login_attempts SET
                       failures = CASE
                         WHEN window_start < now() - make_interval(mins => $2)
                         THEN 1 ELSE failures + 1 END,
                       window_start = CASE
                         WHEN window_start < now() - make_interval(mins => $2)
                         THEN now() ELSE window_start END
                     WHERE key = $1
                     RETURNING failures",
                    &[key, &WINDOW_MINUTES],
                )
                .await?
                .get(0);
            if failures >= MAX_FAILURES {
                tx.execute(
                    "UPDATE login_attempts
                        SET blocked_until = now() + make_interval(secs => $2)
                      WHERE key = $1",
                    &[key, &(block_seconds(failures) as f64)],
                )
                .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn clear(&self, db: &Object) -> Result<(), ApiError> {
        clear(db, &self.account).await?;
        clear(db, &self.ip).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_block_grows_with_the_failure_count() {
        assert_eq!(block_seconds(5), 60);
        assert_eq!(block_seconds(6), 300);
        assert_eq!(block_seconds(10), 1800);
        assert_eq!(block_seconds(100), 1800);
    }

    #[test]
    fn an_ipv6_address_counts_as_its_64() {
        assert_eq!(
            ip_key("2001:db8:1:2:aaaa::1"),
            ip_key("2001:db8:1:2:bbbb:cccc:dddd:eeee")
        );
        assert_eq!(ip_key("2001:db8:1:2::9"), "ip:2001:db8:1:2::/64");
        assert_ne!(ip_key("2001:db8:1:2::1"), ip_key("2001:db8:1:3::1"));
        assert_eq!(ip_key("::ffff:203.0.113.7"), "ip:203.0.113.7");
        assert_eq!(ip_key("203.0.113.7"), "ip:203.0.113.7");
    }
}
