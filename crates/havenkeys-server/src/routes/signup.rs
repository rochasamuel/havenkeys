//! Self-service signup (spec 2026-10-07 §4): verify an email with a code,
//! then issue the same invite the admin CLI would.
//!
//! Neither route may reveal whether an email has an account. `start`
//! answers `202` with an empty body whether it mailed a code or a "you
//! already have an account" notice, and `verify` answers every failure
//! with one message. A code is six digits with five attempts and fifteen
//! minutes, so a guess has a 1 in 200,000 chance per code; it is stored
//! keyed by the server secret, so a dumped table cannot be brute-forced
//! offline. Whoever reads the mailbox can create the account, which is the
//! same trust the invite mail already places in it.

use crate::auth::rate_limit;
use crate::billing::{self, Actor, Status};
use crate::error::ApiError;
use crate::invite::{self, Invite, SIGNUP_INVITE_TTL_HOURS};
use crate::json::Json;
use crate::mail::templates::{self, Locale};
use crate::mail::Mail;
use crate::routes::auth::client_ip;
use crate::routes::AppState;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use rand::Rng;
use serde::Deserialize;
use sha2::Sha256;
use std::net::SocketAddr;
use subtle::ConstantTimeEq;
use uuid::Uuid;
use zeroize::Zeroizing;

pub const CODE_TTL_MINUTES: i32 = 15;
pub const CODE_MAX_ATTEMPTS: i32 = 5;
pub const STARTS_PER_IP_PER_HOUR: i32 = 5;
pub const STARTS_PER_EMAIL_PER_HOUR: i32 = 3;
/// Shares the address key with `start` (spec §4.4), so the threshold must
/// leave room for one start plus a code's five attempts.
pub const VERIFY_FAILURES_PER_IP_PER_HOUR: i32 = 10;
const WINDOW_MINUTES: i32 = 60;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartRequest {
    email: String,
    locale: String,
    /// The terms version the user accepted: the terms page's date,
    /// `YYYY-MM-DD`. Recorded on the account (LGPD consent).
    accepted_terms: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifyRequest {
    email: String,
    code: String,
}

/// Six digits from the CSPRNG, zero-padded. `gen_range` is unbiased.
fn generate_code() -> Zeroizing<String> {
    let n: u32 = rand::thread_rng().gen_range(0..1_000_000);
    Zeroizing::new(format!("{n:06}"))
}

/// `HMAC-SHA-256(SERVER_SECRET, code)`: what the table holds.
pub fn code_hash(secret: &[u8; 32], code: &str) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("any key length is accepted");
    mac.update(code.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// The per-email counter's key: keyed by the server secret, so a reader of
/// `login_attempts` cannot confirm a guessed address with a dictionary.
fn email_key(secret: &[u8; 32], email: &str) -> String {
    let digest = code_hash(secret, email);
    format!(
        "signup-email:{}",
        data_encoding::HEXLOWER.encode(&digest[..16])
    )
}

fn address_key(ip: &str) -> String {
    format!("signup-{}", rate_limit::ip_key(ip))
}

fn check_terms_version(raw: &str) -> Result<&str, ApiError> {
    const BAD: ApiError = ApiError::InvalidRequest("acceptedTerms is not valid");
    let b = raw.as_bytes();
    let shaped = b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| matches!(i, 4 | 7) || c.is_ascii_digit());
    if shaped {
        Ok(raw)
    } else {
        Err(BAD)
    }
}

pub async fn start(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<StartRequest>,
) -> Result<(StatusCode, axum::Json<serde_json::Value>), ApiError> {
    // 404 unless signup is open: the route does not exist on a self-hosted
    // server (spec §4.1).
    let (Some(mailer), Some(_)) = (&state.mailer, &state.signup_url) else {
        return Err(ApiError::NotFound);
    };
    let locale =
        Locale::parse(&req.locale).ok_or(ApiError::InvalidRequest("locale is not valid"))?;
    let terms = check_terms_version(&req.accepted_terms)?;
    let email = crate::email::normalize(&req.email).map_err(ApiError::InvalidRequest)?;

    let db = state.pool.get().await?;
    let ip = client_ip(&state, &headers, peer);
    rate_limit::charge_window(
        &db,
        &address_key(&ip),
        STARTS_PER_IP_PER_HOUR,
        WINDOW_MINUTES,
    )
    .await?;
    rate_limit::charge_window(
        &db,
        &email_key(&state.server_secret, &email),
        STARTS_PER_EMAIL_PER_HOUR,
        WINDOW_MINUTES,
    )
    .await?;

    let existing: Option<String> = db
        .query_opt(
            "SELECT status FROM accounts WHERE email_normalized = $1",
            &[&email],
        )
        .await?
        .map(|r| r.get(0));
    let (subject, body) = match existing.as_deref() {
        // No account, or one still waiting for its invite: a code. The
        // upsert replaces a live code and resets its attempts.
        None | Some("invited") => {
            let code = generate_code();
            db.execute(
                "INSERT INTO signup_codes
                   (email_normalized, code_hash, locale, terms_version, expires_at, attempts, created_at)
                 VALUES ($1, $2, $3, $4, now() + make_interval(mins => $5), 0, now())
                 ON CONFLICT (email_normalized) DO UPDATE SET
                   code_hash = EXCLUDED.code_hash,
                   locale = EXCLUDED.locale,
                   terms_version = EXCLUDED.terms_version,
                   expires_at = EXCLUDED.expires_at,
                   attempts = 0,
                   created_at = now()",
                &[
                    &email,
                    &code_hash(&state.server_secret, &code),
                    &locale.as_str(),
                    &terms,
                    &CODE_TTL_MINUTES,
                ],
            )
            .await?;
            templates::signup_code(locale, &code)
        }
        Some(_) => {
            // Clear a code left from when the account was still invited,
            // so both branches do one write.
            db.execute(
                "DELETE FROM signup_codes WHERE email_normalized = $1",
                &[&email],
            )
            .await?;
            templates::already_registered(locale)
        }
    };
    // Both branches send one mail, so a failure answers 503 in both and a
    // success 202 in both: the outcome says nothing about the account.
    if mailer
        .send(Mail {
            to: email,
            subject,
            body,
        })
        .await
        .is_err()
    {
        return Err(ApiError::Unavailable);
    }
    tracing::info!(outcome = "started", "signup");
    Ok((StatusCode::ACCEPTED, axum::Json(serde_json::json!({}))))
}

pub async fn verify(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<VerifyRequest>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    const BAD: ApiError = ApiError::InvalidRequest("code is not valid");
    let (Some(mailer), Some(public_url)) = (&state.mailer, &state.signup_url) else {
        return Err(ApiError::NotFound);
    };
    let email = crate::email::normalize(&req.email).map_err(|_| BAD)?;
    if req.code.len() != 6 || !req.code.bytes().all(|b| b.is_ascii_digit()) {
        return Err(BAD);
    }

    let mut db = state.pool.get().await?;
    let ip = address_key(&client_ip(&state, &headers, peer));
    rate_limit::over_window(&db, &ip, VERIFY_FAILURES_PER_IP_PER_HOUR, WINDOW_MINUTES).await?;

    let tx = db.transaction().await?;
    // The row lock serializes two verifies of one code: the second finds
    // the row gone (spent) or sees the attempt the first counted.
    let row = tx
        .query_opt(
            "SELECT code_hash, locale, terms_version, expires_at < now(), attempts
               FROM signup_codes WHERE email_normalized = $1 FOR UPDATE",
            &[&email],
        )
        .await?;
    let Some(row) = row else {
        drop(tx);
        if rate_limit::charge_window(&db, &ip, VERIFY_FAILURES_PER_IP_PER_HOUR, WINDOW_MINUTES)
            .await
            .is_err()
        {
            tracing::warn!(kind = "rate_limit", "database error");
        }
        return Err(BAD);
    };
    let stored: Vec<u8> = row.get(0);
    let locale = Locale::from_db(Some(row.get::<_, String>(1).as_str()));
    let terms: String = row.get(2);
    let expired: bool = row.get(3);
    let attempts: i32 = row.get(4);
    let offered = code_hash(&state.server_secret, &req.code);
    let matches = stored.len() == offered.len() && stored.ct_eq(&offered).unwrap_u8() == 1;
    if expired || attempts >= CODE_MAX_ATTEMPTS || !matches {
        if expired || attempts + 1 >= CODE_MAX_ATTEMPTS {
            tx.execute(
                "DELETE FROM signup_codes WHERE email_normalized = $1",
                &[&email],
            )
            .await?;
        } else {
            tx.execute(
                "UPDATE signup_codes SET attempts = attempts + 1 WHERE email_normalized = $1",
                &[&email],
            )
            .await?;
        }
        tx.commit().await?;
        if rate_limit::charge_window(&db, &ip, VERIFY_FAILURES_PER_IP_PER_HOUR, WINDOW_MINUTES)
            .await
            .is_err()
        {
            tracing::warn!(kind = "rate_limit", "database error");
        }
        tracing::info!(outcome = "rejected", "signup verify");
        return Err(BAD);
    }

    tx.execute(
        "DELETE FROM signup_codes WHERE email_normalized = $1",
        &[&email],
    )
    .await?;
    let secret = invite::generate_secret();
    let expires = Utc::now() + Duration::hours(SIGNUP_INVITE_TTL_HOURS);
    let existing = tx
        .query_opt(
            "SELECT id, status FROM accounts WHERE email_normalized = $1 FOR UPDATE",
            &[&email],
        )
        .await?;
    let account = match existing {
        None => {
            let id = Uuid::new_v4();
            tx.execute(
                "INSERT INTO accounts
                   (id, email_normalized, status, invite_hash, invite_expires_at, created_at,
                    created_by, terms_version, terms_accepted_at, locale)
                 VALUES ($1, $2, 'invited', $3, $4, now(), 'signup', $5, now(), $6)",
                &[
                    &id,
                    &email,
                    &invite::hash(&secret).to_vec(),
                    &expires,
                    &terms,
                    &locale.as_str(),
                ],
            )
            .await?;
            billing::set_status(&tx, id, Actor::System, Status::Trialing, None, "signup").await?;
            id
        }
        Some(row) if row.get::<_, String>(1) == "invited" => {
            // An invite already issued (by the admin, or by an earlier
            // signup) is replaced; the old one stops working. The plan row
            // is kept if there is one: the operator chose it.
            let id: Uuid = row.get(0);
            tx.execute(
                "UPDATE accounts
                    SET invite_hash = $2, invite_expires_at = $3,
                        terms_version = $4, terms_accepted_at = now(), locale = $5
                  WHERE id = $1",
                &[
                    &id,
                    &invite::hash(&secret).to_vec(),
                    &expires,
                    &terms,
                    &locale.as_str(),
                ],
            )
            .await?;
            if billing::load(&tx, id).await?.is_none() {
                billing::set_status(&tx, id, Actor::System, Status::Trialing, None, "signup")
                    .await?;
            }
            id
        }
        // Activated between start and verify: the code was for an account
        // that no longer needs one. Same answer as a wrong code.
        Some(_) => {
            tx.commit().await?;
            return Err(BAD);
        }
    };
    tx.commit().await?;

    let encoded = invite::encode(&Invite {
        server: public_url.clone(),
        email: email.clone(),
        account,
        secret: secret.to_string(),
    });
    // The page shows the invite, so a mail failure is not the user's
    // problem; it is logged by kind inside the mailer.
    let (subject, body) = templates::signup_invite(locale, &encoded);
    let _ = mailer
        .send(Mail {
            to: email,
            subject,
            body,
        })
        .await;
    tracing::info!(account_id = %account, outcome = "verified", "signup");
    Ok(axum::Json(serde_json::json!({ "invite": encoded })))
}
