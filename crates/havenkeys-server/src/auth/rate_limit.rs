//! Login rate limiting.
//!
//! Counters are kept per account and per source address, so one noisy network
//! cannot lock out an account it does not own, and one account cannot be
//! ground down from many addresses. The key for an unknown email is the decoy
//! account id (`routes::auth`), which means probing a non-existent account is
//! rate limited exactly like probing a real one.

use crate::error::ApiError;
use deadpool_postgres::Object;

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
}
