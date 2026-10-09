//! Plans and entitlement (spec 2026-10-07 §5).
//!
//! `entitlement` is the only place that decides whether an account may write.
//! It is computed from the stored row and a clock, never flipped by a job
//! that has to fire on time; and `set_status` is the only writer of the
//! stored row, so every change leaves a `billing_events` line.

use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use uuid::Uuid;

pub mod jobs;
pub mod provider;

pub const PLAN_PERSONAL: &str = "personal";
pub const TRIAL_DAYS: i32 = 14;
pub const GRACE_DAYS: i32 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum Status {
    Trialing,
    Active,
    PastDue,
    Frozen,
    Complimentary,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trialing => "trialing",
            Self::Active => "active",
            Self::PastDue => "past_due",
            Self::Frozen => "frozen",
            Self::Complimentary => "complimentary",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "trialing" => Self::Trialing,
            "active" => Self::Active,
            "past_due" => Self::PastDue,
            "frozen" => Self::Frozen,
            "complimentary" => Self::Complimentary,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    pub status: Status,
    pub trial_ends_at: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub grace_ends_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entitlement {
    Full,
    Frozen,
}

impl Entitlement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Frozen => "frozen",
        }
    }
}

/// The one decision (spec §5.3). A date that has arrived counts as past.
pub fn entitlement(sub: &Subscription, now: DateTime<Utc>) -> Entitlement {
    let ended = |at: Option<DateTime<Utc>>| at.is_some_and(|t| t <= now);
    match sub.status {
        Status::Complimentary => Entitlement::Full,
        Status::Active if ended(sub.current_period_end) => Entitlement::Frozen,
        Status::Active => Entitlement::Full,
        Status::Trialing if ended(sub.trial_ends_at) => Entitlement::Frozen,
        Status::Trialing => Entitlement::Full,
        // No grace recorded means no grace: an operator who wants the
        // account open sets the date.
        Status::PastDue if sub.grace_ends_at.is_some() && !ended(sub.grace_ends_at) => {
            Entitlement::Full
        }
        Status::PastDue => Entitlement::Frozen,
        Status::Frozen => Entitlement::Frozen,
    }
}

/// An account without a row is one from before plans existed, which the
/// migration made complimentary; a row that is missing anyway is a bug,
/// and a bug must not hold a vault hostage.
pub fn entitlement_of(sub: Option<&Subscription>, now: DateTime<Utc>) -> Entitlement {
    sub.map_or(Entitlement::Full, |s| entitlement(s, now))
}

/// `status, trial_ends_at, current_period_end, grace_ends_at` read from a
/// row starting at column `first`; `None` when the status column is NULL
/// (a LEFT JOIN that found no row).
pub fn from_row(row: &tokio_postgres::Row, first: usize) -> Option<Subscription> {
    let status: Option<String> = row.get(first);
    let status = Status::parse(&status?)?;
    Some(Subscription {
        status,
        trial_ends_at: row.get(first + 1),
        current_period_end: row.get(first + 2),
        grace_ends_at: row.get(first + 3),
    })
}

pub async fn load(
    db: &impl GenericClient,
    account_id: Uuid,
) -> Result<Option<Subscription>, tokio_postgres::Error> {
    Ok(db
        .query_opt(
            "SELECT status, trial_ends_at, current_period_end, grace_ends_at
               FROM subscriptions WHERE account_id = $1",
            &[&account_id],
        )
        .await?
        .and_then(|row| from_row(&row, 0)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    Admin,
    System,
    Provider,
}

impl Actor {
    fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::System => "system",
            Self::Provider => "provider",
        }
    }
}

/// Set an account's plan status and record the change. `until` lands in
/// the column the status reads: `trial_ends_at` for trialing,
/// `current_period_end` for active, `grace_ends_at` for past_due; it is
/// ignored for frozen and complimentary. Provider columns are untouched.
pub async fn set_status(
    db: &impl GenericClient,
    account_id: Uuid,
    actor: Actor,
    status: Status,
    until: Option<DateTime<Utc>>,
    reason: &str,
) -> Result<(), tokio_postgres::Error> {
    let old: Option<String> = db
        .query_opt(
            "SELECT status FROM subscriptions WHERE account_id = $1 FOR UPDATE",
            &[&account_id],
        )
        .await?
        .map(|r| r.get(0));
    let (trial, period, grace) = match status {
        Status::Trialing => (until, None, None),
        Status::Active => (None, until, None),
        Status::PastDue => (None, None, until),
        Status::Frozen | Status::Complimentary => (None, None, None),
    };
    db.execute(
        "INSERT INTO subscriptions
           (account_id, plan, status, trial_ends_at, current_period_end, grace_ends_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, now())
         ON CONFLICT (account_id) DO UPDATE SET
           status = EXCLUDED.status,
           trial_ends_at = EXCLUDED.trial_ends_at,
           current_period_end = EXCLUDED.current_period_end,
           grace_ends_at = EXCLUDED.grace_ends_at,
           updated_at = now()",
        &[
            &account_id,
            &PLAN_PERSONAL,
            &status.as_str(),
            &trial,
            &period,
            &grace,
        ],
    )
    .await?;
    db.execute(
        "INSERT INTO billing_events (account_id, old_status, new_status, actor, reason)
         VALUES ($1, $2, $3, $4, $5)",
        &[
            &account_id,
            &old,
            &status.as_str(),
            &actor.as_str(),
            &reason,
        ],
    )
    .await?;
    Ok(())
}

/// An audit line that changes nothing (old and new status equal, actor
/// `system`). The trial emails use it to send once per account.
pub async fn note(
    db: &impl GenericClient,
    account_id: Uuid,
    reason: &str,
) -> Result<(), tokio_postgres::Error> {
    db.execute(
        "INSERT INTO billing_events (account_id, old_status, new_status, actor, reason)
         SELECT account_id, status, status, 'system', $2
           FROM subscriptions WHERE account_id = $1",
        &[&account_id, &reason],
    )
    .await?;
    Ok(())
}

/// What the apps are told on login and sync (spec §5.4).
pub fn account_json(sub: Option<&Subscription>, now: DateTime<Utc>) -> serde_json::Value {
    let status = sub.map_or(Status::Complimentary, |s| s.status);
    serde_json::json!({
        "status": status.as_str(),
        "entitlement": entitlement_of(sub, now).as_str(),
        "trialEndsAt": sub.and_then(|s| s.trial_ends_at).map(|t| t.to_rfc3339()),
        "periodEnd": sub.and_then(|s| s.current_period_end).map(|t| t.to_rfc3339()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, day, 12, 0, 0).unwrap()
    }

    fn sub(status: Status) -> Subscription {
        Subscription {
            status,
            trial_ends_at: None,
            current_period_end: None,
            grace_ends_at: None,
        }
    }

    #[test]
    fn complimentary_is_always_full() {
        assert_eq!(
            entitlement(&sub(Status::Complimentary), at(1)),
            Entitlement::Full
        );
    }

    #[test]
    fn active_is_full_until_its_period_ends() {
        let mut s = sub(Status::Active);
        assert_eq!(
            entitlement(&s, at(1)),
            Entitlement::Full,
            "no period end: paid"
        );
        s.current_period_end = Some(at(10));
        assert_eq!(entitlement(&s, at(9)), Entitlement::Full);
        assert_eq!(entitlement(&s, at(10)), Entitlement::Frozen);
    }

    #[test]
    fn trialing_is_full_until_the_trial_ends_and_before_activation() {
        let mut s = sub(Status::Trialing);
        assert_eq!(
            entitlement(&s, at(1)),
            Entitlement::Full,
            "not activated yet"
        );
        s.trial_ends_at = Some(at(15));
        assert_eq!(entitlement(&s, at(14)), Entitlement::Full);
        assert_eq!(entitlement(&s, at(15)), Entitlement::Frozen);
    }

    #[test]
    fn past_due_is_full_only_during_grace() {
        let mut s = sub(Status::PastDue);
        assert_eq!(
            entitlement(&s, at(1)),
            Entitlement::Frozen,
            "no grace recorded"
        );
        s.grace_ends_at = Some(at(8));
        assert_eq!(entitlement(&s, at(7)), Entitlement::Full);
        assert_eq!(entitlement(&s, at(8)), Entitlement::Frozen);
    }

    #[test]
    fn frozen_is_frozen() {
        assert_eq!(
            entitlement(&sub(Status::Frozen), at(1)),
            Entitlement::Frozen
        );
    }

    #[test]
    fn a_missing_row_is_full() {
        assert_eq!(entitlement_of(None, at(1)), Entitlement::Full);
    }

    #[test]
    fn status_round_trips_through_its_name() {
        for s in [
            Status::Trialing,
            Status::Active,
            Status::PastDue,
            Status::Frozen,
            Status::Complimentary,
        ] {
            assert_eq!(Status::parse(s.as_str()), Some(s));
        }
        assert_eq!(Status::parse("whatever"), None);
    }

    #[test]
    fn account_json_names_the_entitlement_and_dates() {
        let mut s = sub(Status::Trialing);
        s.trial_ends_at = Some(at(15));
        let v = account_json(Some(&s), at(1));
        assert_eq!(v["status"], "trialing");
        assert_eq!(v["entitlement"], "full");
        assert_eq!(v["trialEndsAt"], at(15).to_rfc3339());
        assert!(v["periodEnd"].is_null());
        let v = account_json(Some(&s), at(20));
        assert_eq!(v["entitlement"], "frozen");
        let v = account_json(None, at(1));
        assert_eq!(v["status"], "complimentary");
        assert_eq!(v["entitlement"], "full");
    }
}
