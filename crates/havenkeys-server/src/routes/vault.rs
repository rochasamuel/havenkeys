//! The vault header: the wrapped vault key and its attestation.
//!
//! The server stores it as bytes and never parses it. What it does enforce is
//! ordering — a header may only move one revision forward, and its key scheme
//! may never go backwards — so a client that went away and came back cannot
//! be handed an older header than the one it already trusts. The client
//! verifies the attestation itself regardless (docs/crypto.md).

use crate::auth::Session;
use crate::b64::Blob;
use crate::error::ApiError;
use crate::json::Json;
use crate::limits::MAX_HEADER_BYTES;
use crate::routes::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeaderUpdate {
    header: Blob,
    header_revision: i64,
    key_scheme: i16,
}

pub async fn get_header(
    State(state): State<AppState>,
    session: Session,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let db = state.pool.get().await?;
    let row = db
        .query_opt(
            "SELECT header, header_revision, key_scheme FROM vaults WHERE id = $1",
            &[&session.vault_id],
        )
        .await?
        .ok_or(ApiError::NotFound)?;
    let header: Vec<u8> = row.get(0);
    Ok(axum::Json(serde_json::json!({
        "header": Blob(header),
        "headerRevision": row.get::<_, i64>(1),
        "keyScheme": row.get::<_, i16>(2),
    })))
}

pub async fn put_header(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<HeaderUpdate>,
) -> Result<StatusCode, ApiError> {
    if req.header.is_empty() || req.header.len() > MAX_HEADER_BYTES {
        return Err(ApiError::InvalidRequest("header is not valid"));
    }
    let mut db = state.pool.get().await?;
    let tx = db.transaction().await?;
    let row = tx
        .query_opt(
            "SELECT header_revision, key_scheme FROM vaults WHERE id = $1 FOR UPDATE",
            &[&session.vault_id],
        )
        .await?
        .ok_or(ApiError::NotFound)?;
    let current_revision: i64 = row.get(0);
    let current_scheme: i16 = row.get(1);

    if req.key_scheme < current_scheme {
        return Err(ApiError::InvalidRequest("key scheme cannot go backwards"));
    }
    if req.header_revision != current_revision + 1 {
        return Err(ApiError::Conflict);
    }

    tx.execute(
        "UPDATE vaults SET header = $2, header_revision = $3, key_scheme = $4 WHERE id = $1",
        &[
            &session.vault_id,
            &req.header.0,
            &req.header_revision,
            &req.key_scheme,
        ],
    )
    .await?;
    tx.commit().await?;
    tracing::info!(
        account_id = %session.account_id,
        header_revision = req.header_revision,
        "vault header updated"
    );
    Ok(StatusCode::NO_CONTENT)
}
