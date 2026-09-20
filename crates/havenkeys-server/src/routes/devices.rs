//! The devices an account has signed in from, and how to cut one off.
//!
//! Revocation must bite immediately, not at token expiry: a device the user
//! has lost is exactly the case this exists for. So it marks the device and
//! deletes its sessions in one transaction, and the session extractor rejects
//! a revoked device on the very next request.

use crate::auth::Session;
use crate::error::ApiError;
use crate::routes::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

pub async fn list(
    State(state): State<AppState>,
    session: Session,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    let db = state.pool.get().await?;
    let rows = db
        .query(
            "SELECT id, name, created_at, last_seen_at
               FROM devices
              WHERE account_id = $1 AND revoked_at IS NULL
              ORDER BY created_at",
            &[&session.account_id],
        )
        .await?;
    let devices: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let id: Uuid = row.get(0);
            let created: chrono::DateTime<chrono::Utc> = row.get(2);
            let seen: Option<chrono::DateTime<chrono::Utc>> = row.get(3);
            serde_json::json!({
                "id": id,
                "name": row.get::<_, String>(1),
                "createdAt": created.to_rfc3339(),
                "lastSeenAt": seen.map(|t| t.to_rfc3339()),
                "current": id == session.device_id,
            })
        })
        .collect();
    Ok(axum::Json(serde_json::json!(devices)))
}

pub async fn revoke(
    State(state): State<AppState>,
    session: Session,
    Path(device_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let mut db = state.pool.get().await?;
    let tx = db.transaction().await?;
    // Scoped by the session's account: another account's device answers 404,
    // which is also what a device that does not exist answers. The list of
    // device ids is not an oracle.
    let marked = tx
        .execute(
            "UPDATE devices SET revoked_at = now()
              WHERE id = $1 AND account_id = $2 AND revoked_at IS NULL",
            &[&device_id, &session.account_id],
        )
        .await?;
    if marked == 0 {
        return Err(ApiError::NotFound);
    }
    tx.execute("DELETE FROM sessions WHERE device_id = $1", &[&device_id])
        .await?;
    tx.commit().await?;
    tracing::info!(
        account_id = %session.account_id,
        device_id = %device_id,
        "device revoked"
    );
    Ok(StatusCode::NO_CONTENT)
}
