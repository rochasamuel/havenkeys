//! The vault header: the wrapped vault key and its attestation.
//!
//! Served here; changed only through `account::change_credentials`, together
//! with the login verifier.

use crate::auth::Session;
use crate::b64::Blob;
use crate::error::ApiError;
use crate::routes::AppState;
use axum::extract::State;

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
