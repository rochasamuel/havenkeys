//! Signing in a new device from an approving one
//! (spec 2026-10-03-phone-approved-sign-in §3, §5).
//!
//! The server relays an envelope it cannot open and issues the new device's
//! session when a signed-in device of the same account approves. Unknown,
//! expired, used, denied and other-account pairings all answer 404.

use crate::auth::{self, rate_limit, Session};
use crate::error::ApiError;
use crate::json::Json;
use crate::limits::{
    MAX_PAIRING_ENVELOPE_BYTES, PAIRINGS_PER_IP, PAIRING_MAX_AGE_MINUTES, PAIRING_TTL_SECONDS,
    PENDING_PAIRINGS_PER_IP,
};
use crate::routes::auth::{clean_device_name, client_ip, register_device};
use crate::routes::AppState;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, StatusCode};
use data_encoding::{BASE64, BASE64URL_NOPAD};
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use subtle::ConstantTimeEq;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateRequest {
    device_id: Uuid,
    device_name: String,
    public_key: String,
    claim_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApproveRequest {
    envelope: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaimRequest {
    claim_secret: String,
}

fn fixed32(raw: &str, bad: &'static str) -> Result<[u8; 32], ApiError> {
    if raw.len() > 64 {
        return Err(ApiError::InvalidRequest(bad));
    }
    BASE64URL_NOPAD
        .decode(raw.as_bytes())
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or(ApiError::InvalidRequest(bad))
}

/// 16 random bytes; a path segment that is anything else is simply gone.
fn pairing_id(raw: &str) -> Result<String, ApiError> {
    let ok = raw.len() == 22
        && BASE64URL_NOPAD
            .decode(raw.as_bytes())
            .is_ok_and(|b| b.len() == 16);
    if ok {
        Ok(raw.to_string())
    } else {
        Err(ApiError::NotFound)
    }
}

pub async fn create(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<CreateRequest>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let device_name = clean_device_name(&req.device_name)?;
    let public_key = fixed32(&req.public_key, "publicKey is not valid")?;
    let claim_hash = fixed32(&req.claim_hash, "claimHash is not valid")?;
    let ip = client_ip(&state, &headers, peer);
    let db = state.pool.get().await?;

    // No periodic task: old rows go here, whatever their state.
    db.execute(
        "DELETE FROM pairings WHERE created_at < now() - make_interval(mins => $1)",
        &[&PAIRING_MAX_AGE_MINUTES],
    )
    .await?;
    let counts = db
        .query_one(
            "SELECT count(*),
                    count(*) FILTER (WHERE state = 'pending' AND expires_at > now())
               FROM pairings WHERE ip = $1",
            &[&ip],
        )
        .await?;
    let (total, pending): (i64, i64) = (counts.get(0), counts.get(1));
    if total >= PAIRINGS_PER_IP || pending >= PENDING_PAIRINGS_PER_IP {
        return Err(ApiError::RateLimited);
    }

    let mut raw = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut raw);
    let id = BASE64URL_NOPAD.encode(&raw);
    let location = state.locator.as_ref().and_then(|l| l.locate(&ip));
    let row = db
        .query_one(
            "INSERT INTO pairings
               (id, state, device_id, device_name, public_key, claim_hash, ip, location, expires_at)
             VALUES ($1, 'pending', $2, $3, $4, $5, $6, $7, now() + make_interval(secs => $8))
             RETURNING expires_at",
            &[
                &id,
                &req.device_id,
                &device_name,
                &public_key.as_slice(),
                &claim_hash.as_slice(),
                &ip,
                &location,
                &PAIRING_TTL_SECONDS,
            ],
        )
        .await?;
    let expires: chrono::DateTime<chrono::Utc> = row.get(0);
    tracing::info!(outcome = "created", "pairing");
    Ok(axum::Json(serde_json::json!({
        "pairingId": id,
        "expiresAt": expires.to_rfc3339(),
    })))
}

pub async fn details(
    State(state): State<AppState>,
    session: Session,
    Path(raw): Path<String>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let id = pairing_id(&raw)?;
    let db = state.pool.get().await?;
    // Reading binds the pairing to this account: no other account can see,
    // approve or deny it afterwards.
    let row = db
        .query_opt(
            "UPDATE pairings SET account_id = $2
              WHERE id = $1 AND state = 'pending' AND expires_at > now()
                AND (account_id IS NULL OR account_id = $2)
             RETURNING device_name, ip, location, created_at, expires_at",
            &[&id, &session.account_id],
        )
        .await?
        .ok_or(ApiError::NotFound)?;
    let created: chrono::DateTime<chrono::Utc> = row.get(3);
    let expires: chrono::DateTime<chrono::Utc> = row.get(4);
    Ok(axum::Json(serde_json::json!({
        "deviceName": row.get::<_, String>(0),
        "ip": row.get::<_, String>(1),
        "location": row.get::<_, Option<String>>(2),
        "createdAt": created.to_rfc3339(),
        "expiresAt": expires.to_rfc3339(),
    })))
}

pub async fn approve(
    State(state): State<AppState>,
    session: Session,
    Path(raw): Path<String>,
    Json(req): Json<ApproveRequest>,
) -> Result<StatusCode, ApiError> {
    let id = pairing_id(&raw)?;
    const BAD: ApiError = ApiError::InvalidRequest("envelope is not valid");
    if req.envelope.len() > MAX_PAIRING_ENVELOPE_BYTES / 3 * 4 + 4 {
        return Err(BAD);
    }
    let envelope = BASE64.decode(req.envelope.as_bytes()).map_err(|_| BAD)?;
    if envelope.is_empty() || envelope.len() > MAX_PAIRING_ENVELOPE_BYTES {
        return Err(BAD);
    }

    let mut db = state.pool.get().await?;
    let tx = db.transaction().await?;
    let row = tx
        .query_opt(
            "SELECT device_id, device_name FROM pairings
              WHERE id = $1 AND state = 'pending' AND expires_at > now()
                AND (account_id IS NULL OR account_id = $2)
              FOR UPDATE",
            &[&id, &session.account_id],
        )
        .await?
        .ok_or(ApiError::NotFound)?;
    let device_id: Uuid = row.get(0);
    let device_name: String = row.get(1);
    if device_id == session.device_id {
        return Err(ApiError::InvalidRequest("a device cannot approve itself"));
    }
    register_device(&tx, session.account_id, device_id, &device_name).await?;
    tx.execute(
        "UPDATE devices SET approved_by = $2 WHERE id = $1",
        &[&device_id, &session.device_id],
    )
    .await?;
    tx.execute("DELETE FROM sessions WHERE device_id = $1", &[&device_id])
        .await?;
    let (token, expires) = auth::issue_token(&tx, session.account_id, device_id).await?;
    tx.execute(
        "UPDATE pairings
            SET state = 'approved', account_id = $2, envelope = $3,
                token = $4, token_expires_at = $5
          WHERE id = $1",
        &[
            &id,
            &session.account_id,
            &envelope,
            &token.as_str(),
            &expires,
        ],
    )
    .await?;
    tx.commit().await?;
    tracing::info!(account_id = %session.account_id, device_id = %device_id, outcome = "approved", "pairing");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn deny(
    State(state): State<AppState>,
    session: Session,
    Path(raw): Path<String>,
) -> Result<StatusCode, ApiError> {
    let id = pairing_id(&raw)?;
    let db = state.pool.get().await?;
    let changed = db
        .execute(
            "UPDATE pairings SET state = 'denied', account_id = $2
              WHERE id = $1 AND state = 'pending' AND expires_at > now()
                AND (account_id IS NULL OR account_id = $2)",
            &[&id, &session.account_id],
        )
        .await?;
    if changed == 0 {
        return Err(ApiError::NotFound);
    }
    tracing::info!(account_id = %session.account_id, outcome = "denied", "pairing");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn claim(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(raw): Path<String>,
    Json(req): Json<ClaimRequest>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let id = pairing_id(&raw)?;
    let secret = fixed32(&req.claim_secret, "claimSecret is not valid")?;
    let ip_key = rate_limit::ip_key(&client_ip(&state, &headers, peer));
    let mut db = state.pool.get().await?;
    rate_limit::check(&db, &ip_key).await?;

    // Read and consume in one transaction under a row lock, so two claims
    // racing with the right secret cannot both receive the token.
    let tx = db.transaction().await?;
    let row = tx
        .query_opt(
            "SELECT state, claim_hash, expires_at > now(), account_id, envelope, token, token_expires_at
               FROM pairings
              WHERE id = $1 AND created_at > now() - make_interval(mins => $2)
              FOR UPDATE",
            &[&id, &PAIRING_MAX_AGE_MINUTES],
        )
        .await?
        .ok_or(ApiError::NotFound)?;
    let stored: Vec<u8> = row.get(1);
    let given = Sha256::digest(secret);
    if !bool::from(stored.as_slice().ct_eq(given.as_slice())) {
        drop(tx);
        rate_limit::record_failure(&db, &ip_key).await?;
        return Err(ApiError::NotFound);
    }
    let live: bool = row.get(2);
    match row.get::<_, String>(0).as_str() {
        "pending" if live => Ok(axum::Json(serde_json::json!({ "state": "waiting" }))),
        "denied" => {
            tx.execute(
                "UPDATE pairings SET state = 'claimed' WHERE id = $1",
                &[&id],
            )
            .await?;
            tx.commit().await?;
            Ok(axum::Json(serde_json::json!({ "state": "denied" })))
        }
        "approved" => {
            let account_id: Uuid = row.get(3);
            let envelope: Vec<u8> = row.get(4);
            let token: String = row.get(5);
            let expires: chrono::DateTime<chrono::Utc> = row.get(6);
            let vault_id: Uuid = tx
                .query_one(
                    "SELECT id FROM vaults WHERE account_id = $1",
                    &[&account_id],
                )
                .await?
                .get(0);
            // Single use: the token and envelope leave the database here.
            tx.execute(
                "UPDATE pairings SET state = 'claimed', envelope = NULL, token = NULL
                  WHERE id = $1 AND state = 'approved'",
                &[&id],
            )
            .await?;
            tx.commit().await?;
            tracing::info!(account_id = %account_id, outcome = "claimed", "pairing");
            Ok(axum::Json(serde_json::json!({
                "state": "approved",
                "token": token,
                "expiresAt": expires.to_rfc3339(),
                "accountId": account_id,
                "vaultId": vault_id,
                "envelope": BASE64.encode(&envelope),
            })))
        }
        _ => Err(ApiError::NotFound),
    }
}
