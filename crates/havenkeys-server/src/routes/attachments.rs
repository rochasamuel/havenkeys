//! Attachment blobs: one sealed file per id, stored beside the items.
//!
//! An upload is stored uncommitted and counts towards the vault's quotas, but
//! it does not bump `vaults.revision`: nothing references it yet. Only a
//! committed row can be downloaded. The server never opens a blob.

use crate::auth::Session;
use crate::b64::Blob;
use crate::error::ApiError;
use crate::json::Json;
use crate::limits::{MAX_ATTACHMENT_BLOB_BYTES, MAX_ATTACHMENT_VAULT_BYTES};
use crate::routes::items::vault_bytes;
use crate::routes::AppState;
use axum::extract::{Path, State};
use deadpool_postgres::Transaction;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PutRequest {
    item_id: Uuid,
    blob: Blob,
}

/// The bytes of every attachment the vault holds, committed or pending.
pub(crate) async fn attachment_bytes(
    tx: &Transaction<'_>,
    vault_id: Uuid,
) -> Result<i64, ApiError> {
    Ok(tx
        .query_one(
            "SELECT coalesce(sum(octet_length(blob)), 0)::bigint
               FROM attachments WHERE vault_id = $1",
            &[&vault_id],
        )
        .await?
        .get(0))
}

pub async fn put(
    State(state): State<AppState>,
    session: Session,
    Path(attachment_id): Path<Uuid>,
    Json(req): Json<PutRequest>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    session.require_full()?;
    if req.blob.len() > MAX_ATTACHMENT_BLOB_BYTES {
        return Err(ApiError::TooLarge);
    }
    let size = req.blob.len() as i64;

    let mut db = state.pool.get().await?;
    let tx = db.transaction().await?;
    // The vault row lock makes the quota check and the insert one decision.
    tx.query_one(
        "SELECT revision FROM vaults WHERE id = $1 FOR UPDATE",
        &[&session.vault_id],
    )
    .await?;

    let existing = tx
        .query_opt(
            "SELECT item_id, blob FROM attachments
              WHERE vault_id = $1 AND attachment_id = $2",
            &[&session.vault_id, &attachment_id],
        )
        .await?;
    if let Some(row) = existing {
        // A retry of the same upload is a no-op; anything else reuses an id.
        let same = row.get::<_, Uuid>(0) == req.item_id && row.get::<_, Vec<u8>>(1) == req.blob.0;
        return if same {
            Ok(axum::Json(serde_json::json!({})))
        } else {
            Err(ApiError::Conflict)
        };
    }

    if attachment_bytes(&tx, session.vault_id).await? + size > MAX_ATTACHMENT_VAULT_BYTES {
        return Err(ApiError::AttachmentsFull);
    }
    if vault_bytes(&tx, session.vault_id).await? + size > state.max_vault_bytes {
        return Err(ApiError::VaultFull);
    }
    tx.execute(
        "INSERT INTO attachments (vault_id, attachment_id, item_id, blob)
         VALUES ($1, $2, $3, $4)",
        &[&session.vault_id, &attachment_id, &req.item_id, &req.blob.0],
    )
    .await?;
    tx.commit().await?;
    tracing::info!(account_id = %session.account_id, "attachment stored");
    Ok(axum::Json(serde_json::json!({})))
}

pub async fn get(
    State(state): State<AppState>,
    session: Session,
    Path(attachment_id): Path<Uuid>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let db = state.pool.get().await?;
    let row = db
        .query_opt(
            "SELECT item_id, blob FROM attachments
              WHERE vault_id = $1 AND attachment_id = $2 AND committed",
            &[&session.vault_id, &attachment_id],
        )
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(axum::Json(serde_json::json!({
        "itemId": row.get::<_, Uuid>(0),
        "blob": Blob(row.get(1)),
    })))
}
