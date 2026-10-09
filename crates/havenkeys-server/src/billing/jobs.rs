//! The daily task (spec 2026-10-07 §4.3, §4.5, §5.6): sweep what expired,
//! delete signups nobody finished, and send the two trial notices.
//!
//! Nothing here changes an entitlement; that is computed on every request.
//! A notice is marked in `billing_events` only after it was sent, so a day
//! without SMTP sends it the next day instead of never.

use super::note;
use crate::mail::templates::{self, Locale};
use crate::mail::{Mail, Mailer};
use crate::routes::signup::ABANDONED_SIGNUP_DAYS;
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use uuid::Uuid;

/// How many days before the end the "ending" notice goes out.
const ENDING_NOTICE_DAYS: i32 = 3;
const ENDING_REASON: &str = "trial_ending_notice";
const ENDED_REASON: &str = "trial_ended_notice";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DailyReport {
    pub codes_swept: u64,
    pub signups_abandoned: u64,
    pub counters_swept: u64,
    pub ending_notices: u64,
    pub ended_notices: u64,
}

pub async fn run_daily(
    db: &impl GenericClient,
    mailer: Option<&dyn Mailer>,
) -> Result<DailyReport, tokio_postgres::Error> {
    let codes_swept = db
        .execute("DELETE FROM signup_codes WHERE expires_at <= now()", &[])
        .await?;
    // A signup account that was never activated holds nothing but an
    // email; the cascade takes its plan row and its events.
    let signups_abandoned = db
        .execute(
            "DELETE FROM accounts
              WHERE status = 'invited' AND created_by = 'signup'
                AND created_at < now() - make_interval(days => $1)
                AND (invite_expires_at IS NULL OR invite_expires_at < now())",
            &[&ABANDONED_SIGNUP_DAYS],
        )
        .await?;
    let counters_swept = db
        .execute(
            "DELETE FROM login_attempts
              WHERE key LIKE 'signup-%' AND window_start < now() - interval '2 hours'",
            &[],
        )
        .await?;
    let mut report = DailyReport {
        codes_swept,
        signups_abandoned,
        counters_swept,
        ..DailyReport::default()
    };
    let Some(mailer) = mailer else {
        return Ok(report);
    };

    let ending = db
        .query(
            "SELECT a.id, a.email_normalized, a.locale, p.trial_ends_at
               FROM subscriptions p JOIN accounts a ON a.id = p.account_id
              WHERE p.status = 'trialing' AND a.status = 'active'
                AND p.trial_ends_at > now()
                AND p.trial_ends_at <= now() + make_interval(days => $1)
                AND NOT EXISTS (SELECT 1 FROM billing_events e
                                 WHERE e.account_id = a.id AND e.reason = $2)",
            &[&ENDING_NOTICE_DAYS, &ENDING_REASON],
        )
        .await?;
    for row in &ending {
        let (id, email, locale, ends) = fields(row);
        let hours = (ends - Utc::now()).num_hours().max(0);
        let days = (hours + 23) / 24;
        let (subject, body) = templates::trial_ending(locale, days);
        if mailer
            .send(Mail {
                to: email,
                subject,
                body,
            })
            .await
            .is_ok()
        {
            note(db, id, ENDING_REASON).await?;
            report.ending_notices += 1;
        }
    }

    let ended = db
        .query(
            "SELECT a.id, a.email_normalized, a.locale, p.trial_ends_at
               FROM subscriptions p JOIN accounts a ON a.id = p.account_id
              WHERE p.status = 'trialing' AND a.status = 'active'
                AND p.trial_ends_at <= now()
                AND NOT EXISTS (SELECT 1 FROM billing_events e
                                 WHERE e.account_id = a.id AND e.reason = $1)",
            &[&ENDED_REASON],
        )
        .await?;
    for row in &ended {
        let (id, email, locale, _) = fields(row);
        let (subject, body) = templates::trial_ended(locale);
        if mailer
            .send(Mail {
                to: email,
                subject,
                body,
            })
            .await
            .is_ok()
        {
            note(db, id, ENDED_REASON).await?;
            report.ended_notices += 1;
        }
    }
    Ok(report)
}

fn fields(row: &tokio_postgres::Row) -> (Uuid, String, Locale, DateTime<Utc>) {
    let locale: Option<String> = row.get(2);
    (
        row.get(0),
        row.get(1),
        Locale::from_db(locale.as_deref()),
        row.get(3),
    )
}
