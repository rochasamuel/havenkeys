//! Changing the account's credentials: the master password, from the
//! server's point of view.
//!
//! A new master password means a new KDF salt, a new auth key and a new key
//! wrap. All three have to move together: a header without the verifier
//! locks every device out of the server, and a verifier without the header
//! leaves other devices unable to open the vault. So this is one route and
//! one transaction.
//!
//! The session alone is not enough to ask. A stolen token could otherwise
//! replace the password and lock the owner out, so the caller also proves it
//! knows the current auth key, which is checked and rate limited exactly
//! like a login. Every other session on the account is deleted: a password
//! is often changed because something leaked.

use crate::auth::{self, rate_limit, Session};
use crate::b64::Blob;
use crate::error::ApiError;
use crate::json::Json;
use crate::limits::MAX_HEADER_BYTES;
use crate::routes::accounts::KdfDto;
use crate::routes::AppState;
use axum::extract::{ConnectInfo, State};
use axum::http::HeaderMap;
use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CredentialChange {
    current_auth_key: String,
    kdf: KdfDto,
    new_auth_key: String,
    header: Blob,
    base_header_revision: i64,
}

pub async fn change_credentials(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    session: Session,
    Json(req): Json<CredentialChange>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    if req.header.is_empty() || req.header.len() > MAX_HEADER_BYTES {
        return Err(ApiError::InvalidRequest("header is not valid"));
    }
    if req.base_header_revision < 0 {
        return Err(ApiError::InvalidRequest("baseHeaderRevision is not valid"));
    }
    let salt = req.kdf.check()?;
    let current = auth::decode_auth_key(&req.current_auth_key)?;
    let new = auth::decode_auth_key(&req.new_auth_key)?;

    let mut db = state.pool.get().await?;
    let account_key = format!("acct:{}", session.account_id);
    let ip_key = format!(
        "ip:{}",
        crate::routes::auth::client_ip(&state, &headers, peer)
    );
    rate_limit::check(&db, &account_key).await?;
    rate_limit::check(&db, &ip_key).await?;

    // Verified before the transaction, like login: Argon2id takes tens of
    // milliseconds and no row lock should be held through it. The verifier is
    // compared again under the lock, so a concurrent change still loses.
    let stored: String = db
        .query_opt(
            "SELECT auth_verifier FROM accounts WHERE id = $1 AND status = 'active'",
            &[&session.account_id],
        )
        .await?
        .and_then(|r| r.get::<_, Option<String>>(0))
        .ok_or(ApiError::Unauthorized)?;
    if !auth::verify_auth_key(stored.clone(), current).await {
        rate_limit::record_failure(&db, &account_key).await?;
        rate_limit::record_failure(&db, &ip_key).await?;
        tracing::info!(account_id = %session.account_id, outcome = "rejected", "credential change");
        return Err(ApiError::Unauthorized);
    }
    let verifier = auth::hash_auth_key(new).await?;

    let tx = db.transaction().await?;
    let locked: String = tx
        .query_one(
            "SELECT auth_verifier FROM accounts WHERE id = $1 FOR UPDATE",
            &[&session.account_id],
        )
        .await?
        .get(0);
    if locked != stored {
        return Err(ApiError::Conflict);
    }
    let current_revision: i64 = tx
        .query_one(
            "SELECT header_revision FROM vaults WHERE id = $1 FOR UPDATE",
            &[&session.vault_id],
        )
        .await?
        .get(0);
    if req.base_header_revision != current_revision {
        return Err(ApiError::Conflict);
    }
    let next = current_revision + 1;
    tx.execute(
        "UPDATE accounts
            SET kdf_algorithm = 'argon2id', kdf_memory_kib = $2, kdf_iterations = $3,
                kdf_parallelism = $4, kdf_salt = $5, auth_verifier = $6
          WHERE id = $1",
        &[
            &session.account_id,
            &req.kdf.memory_kib,
            &req.kdf.iterations,
            &req.kdf.parallelism,
            &salt,
            &verifier,
        ],
    )
    .await?;
    tx.execute(
        "UPDATE vaults SET header = $2, header_revision = $3 WHERE id = $1",
        &[&session.vault_id, &req.header.0, &next],
    )
    .await?;
    tx.execute(
        "DELETE FROM sessions WHERE account_id = $1 AND token_hash <> $2",
        &[&session.account_id, &session.token_hash],
    )
    .await?;
    tx.commit().await?;

    rate_limit::clear(&db, &account_key).await?;
    rate_limit::clear(&db, &ip_key).await?;
    tracing::info!(account_id = %session.account_id, header_revision = next, "credentials changed");
    Ok(axum::Json(serde_json::json!({ "headerRevision": next })))
}
