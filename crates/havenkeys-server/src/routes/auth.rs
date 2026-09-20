//! Sign-in: the parameters a device needs before it can derive anything, and
//! the login that turns a derived auth key into a session.
//!
//! Neither route may reveal whether an email has an account. `auth/params`
//! answers unknown addresses with a decoy account id and salt derived from
//! the server secret — stable per address, so asking twice tells a prober
//! nothing — and `login` spends the same Argon2id time on a dummy verifier.

use crate::auth::{self, rate_limit};
use crate::error::ApiError;
use crate::json::Json;
use crate::limits::{MAX_DEVICES_PER_ACCOUNT, MAX_DEVICE_NAME_CHARS};
use crate::routes::AppState;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use data_encoding::BASE64;
use deadpool_postgres::Object;
use hkdf::Hkdf;
use serde::Deserialize;
use sha2::Sha256;
use std::net::SocketAddr;
use uuid::Uuid;

/// The cost a client should use when it has no stored parameters. Matches
/// `havenkeys-core::crypto::kdf`'s defaults.
const DEFAULT_MEMORY_KIB: i32 = 128 * 1024;
const DEFAULT_ITERATIONS: i32 = 4;
const DEFAULT_PARALLELISM: i32 = 4;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParamsRequest {
    email: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoginRequest {
    email: String,
    auth_key: String,
    device_id: Uuid,
    device_name: String,
}

fn derive(secret: &[u8; 32], domain: &[u8], email: &str) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(domain), secret);
    let mut out = [0u8; 32];
    hk.expand(email.as_bytes(), &mut out)
        .expect("32 bytes is a valid HKDF length");
    out
}

/// The account id an unknown email is answered with: unguessable without the
/// server secret, and the same on every ask.
fn decoy_account(secret: &[u8; 32], email: &str) -> Uuid {
    let bytes = derive(secret, b"havenkeys/params/account", email);
    let mut id = [0u8; 16];
    id.copy_from_slice(&bytes[..16]);
    Uuid::from_bytes(id)
}

fn decoy_salt(secret: &[u8; 32], email: &str) -> Vec<u8> {
    derive(secret, b"havenkeys/params/salt", email)[..16].to_vec()
}

pub async fn params(
    State(state): State<AppState>,
    Json(req): Json<ParamsRequest>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    // An unparseable address gets a decoy too: "that is not an email" would
    // be one more bit than the endpoint should give away.
    let email =
        crate::email::normalize(&req.email).unwrap_or_else(|_| req.email.trim().to_string());
    let db = state.pool.get().await?;
    let row = db
        .query_opt(
            "SELECT id, kdf_memory_kib, kdf_iterations, kdf_parallelism, kdf_salt
               FROM accounts
              WHERE email_normalized = $1 AND status = 'active'",
            &[&email],
        )
        .await?;

    let (account_id, memory, iterations, parallelism, salt) = match row {
        Some(row) => (
            row.get::<_, Uuid>(0),
            row.get::<_, Option<i32>>(1).unwrap_or(DEFAULT_MEMORY_KIB),
            row.get::<_, Option<i32>>(2).unwrap_or(DEFAULT_ITERATIONS),
            row.get::<_, Option<i32>>(3).unwrap_or(DEFAULT_PARALLELISM),
            row.get::<_, Option<Vec<u8>>>(4)
                .unwrap_or_else(|| decoy_salt(&state.server_secret, &email)),
        ),
        None => (
            decoy_account(&state.server_secret, &email),
            DEFAULT_MEMORY_KIB,
            DEFAULT_ITERATIONS,
            DEFAULT_PARALLELISM,
            decoy_salt(&state.server_secret, &email),
        ),
    };

    Ok(axum::Json(serde_json::json!({
        "accountId": account_id,
        "kdf": {
            "algorithm": "argon2id",
            "memoryKib": memory,
            "iterations": iterations,
            "parallelism": parallelism,
            "salt": BASE64.encode(&salt),
        }
    })))
}

pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let email =
        crate::email::normalize(&req.email).unwrap_or_else(|_| req.email.trim().to_string());
    let auth_key = auth::decode_auth_key(&req.auth_key)?;
    let device_name = clean_device_name(&req.device_name)?;
    let db = state.pool.get().await?;

    let row = db
        .query_opt(
            "SELECT id, auth_verifier FROM accounts
              WHERE email_normalized = $1 AND status = 'active'",
            &[&email],
        )
        .await?;
    // The rate-limit key for an unknown email is its decoy id, so probing a
    // non-existent account is throttled exactly like probing a real one.
    let account_id = row
        .as_ref()
        .map(|r| r.get::<_, Uuid>(0))
        .unwrap_or_else(|| decoy_account(&state.server_secret, &email));
    let account_key = format!("acct:{account_id}");
    let ip_key = format!("ip:{}", client_ip(&state, &headers, peer));
    rate_limit::check(&db, &account_key).await?;
    rate_limit::check(&db, &ip_key).await?;

    let verifier = row
        .as_ref()
        .and_then(|r| r.get::<_, Option<String>>(1))
        .unwrap_or_else(|| auth::dummy_verifier().to_string());
    let known = row.is_some();
    // Always verify, so the response time does not depend on whether the
    // account exists.
    let ok = auth::verify_auth_key(verifier, auth_key).await && known;
    if !ok {
        rate_limit::record_failure(&db, &account_key).await?;
        rate_limit::record_failure(&db, &ip_key).await?;
        tracing::info!(account_id = %account_id, outcome = "rejected", "login");
        return Err(ApiError::Unauthorized);
    }

    let vault_id: Uuid = db
        .query_opt(
            "SELECT id FROM vaults WHERE account_id = $1",
            &[&account_id],
        )
        .await?
        .ok_or(ApiError::Unauthorized)?
        .get(0);

    register_device(&db, account_id, req.device_id, &device_name).await?;
    db.execute(
        "DELETE FROM sessions WHERE device_id = $1",
        &[&req.device_id],
    )
    .await?;
    let (token, expires) = auth::issue_token(&db, account_id, req.device_id).await?;

    rate_limit::clear(&db, &account_key).await?;
    rate_limit::clear(&db, &ip_key).await?;
    tracing::info!(account_id = %account_id, device_id = %req.device_id, outcome = "accepted", "login");
    Ok(axum::Json(serde_json::json!({
        "token": token.as_str(),
        "expiresAt": expires.to_rfc3339(),
        "vaultId": vault_id,
    })))
}

pub async fn logout(
    State(state): State<AppState>,
    session: auth::Session,
) -> Result<StatusCode, ApiError> {
    let db = state.pool.get().await?;
    db.execute(
        "DELETE FROM sessions WHERE token_hash = $1",
        &[&session.token_hash],
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Insert the device if it is new, refuse it if it belongs to someone else or
/// has been revoked, and keep the account under its device ceiling.
async fn register_device(
    db: &Object,
    account_id: Uuid,
    device_id: Uuid,
    name: &str,
) -> Result<(), ApiError> {
    let existing = db
        .query_opt(
            "SELECT account_id, revoked_at IS NOT NULL FROM devices WHERE id = $1",
            &[&device_id],
        )
        .await?;
    match existing {
        Some(row) => {
            let owner: Uuid = row.get(0);
            let revoked: bool = row.get(1);
            if owner != account_id || revoked {
                return Err(ApiError::Unauthorized);
            }
            db.execute(
                "UPDATE devices SET name = $2, last_seen_at = now() WHERE id = $1",
                &[&device_id, &name],
            )
            .await?;
        }
        None => {
            let count: i64 = db
                .query_one(
                    "SELECT count(*) FROM devices WHERE account_id = $1 AND revoked_at IS NULL",
                    &[&account_id],
                )
                .await?
                .get(0);
            if count >= MAX_DEVICES_PER_ACCOUNT {
                return Err(ApiError::InvalidRequest(
                    "this account has too many devices; revoke one first",
                ));
            }
            db.execute(
                "INSERT INTO devices (id, account_id, name, created_at, last_seen_at)
                 VALUES ($1, $2, $3, now(), now())",
                &[&device_id, &account_id, &name],
            )
            .await?;
        }
    }
    Ok(())
}

/// A label the user chose. Control characters would end up in the device list
/// and in logs, so they are refused rather than stripped.
fn clean_device_name(raw: &str) -> Result<String, ApiError> {
    const BAD: ApiError = ApiError::InvalidRequest("deviceName is not valid");
    let name = raw.trim();
    if name.is_empty() || name.chars().count() > MAX_DEVICE_NAME_CHARS {
        return Err(BAD);
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(BAD);
    }
    Ok(name.to_string())
}

/// The address a rate-limit counter is keyed by. `X-Forwarded-For` is
/// believed only when the deployment says a proxy sets it; otherwise a client
/// could send the header itself and spread its attempts over invented
/// addresses.
fn client_ip(state: &AppState, headers: &HeaderMap, peer: SocketAddr) -> String {
    if state.trust_forwarded_for {
        if let Some(first) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(str::trim)
            .filter(|v| !v.is_empty() && v.len() <= 64)
        {
            return first.to_string();
        }
    }
    peer.ip().to_string()
}
