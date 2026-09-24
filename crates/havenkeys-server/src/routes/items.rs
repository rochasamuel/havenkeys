//! Writes: the only way item data changes.
//!
//! Concurrency is optimistic and per item. A change carries the revision the
//! client last saw (`baseRevision`, or null for an item it believes is new),
//! and if any of them is stale the **whole batch** is refused and the
//! conflicting items are named. Partial application would leave the client
//! unable to say what happened.
//!
//! The batch runs in one transaction that bumps `vaults.revision` once and
//! stamps every touched row with it, so the pull cursor stays monotonic and
//! no reader ever observes half a batch (design §7.4).

use crate::auth::Session;
use crate::b64::Blob;
use crate::error::{self, ApiError};
use crate::json::Json;
use crate::limits::MAX_CHANGES_PER_BATCH;
use crate::routes::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteRequest {
    changes: Vec<Change>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Change {
    item_id: Uuid,
    /// The revision this client last saw, or `null` for an item it believes
    /// is new. Never trusted: it is compared against the stored row.
    base_revision: Option<i64>,
    #[serde(default)]
    overview: Option<Blob>,
    #[serde(default)]
    details: Option<Blob>,
    #[serde(default)]
    deleted: bool,
}

impl Change {
    /// A change is either a write carrying both blobs or a deletion carrying
    /// neither. Anything else is a malformed request, not a shape to guess at.
    fn check(&self) -> Result<(), ApiError> {
        let well_formed = if self.deleted {
            self.overview.is_none() && self.details.is_none()
        } else {
            self.overview.is_some() && self.details.is_some()
        };
        if well_formed {
            Ok(())
        } else {
            Err(ApiError::InvalidRequest("change is not valid"))
        }
    }
}

pub async fn write(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<WriteRequest>,
) -> Result<Response, ApiError> {
    if req.changes.is_empty() {
        return Err(ApiError::InvalidRequest("changes is empty"));
    }
    if req.changes.len() > MAX_CHANGES_PER_BATCH {
        return Err(ApiError::InvalidRequest("too many changes in one batch"));
    }
    let mut seen = HashSet::with_capacity(req.changes.len());
    for change in &req.changes {
        change.check()?;
        if !seen.insert(change.item_id) {
            // The second mention of an item has no defined base revision.
            return Err(ApiError::InvalidRequest(
                "an item appears twice in one batch",
            ));
        }
    }

    let ids: Vec<Uuid> = req.changes.iter().map(|c| c.item_id).collect();
    let mut db = state.pool.get().await?;
    let tx = db.transaction().await?;
    // The vault row lock serializes concurrent batches for this vault, which
    // is what makes the conflict check and the revision assignment one
    // decision rather than two racing ones.
    tx.query_one(
        "SELECT revision FROM vaults WHERE id = $1 FOR UPDATE",
        &[&session.vault_id],
    )
    .await?;

    let stored: HashMap<Uuid, (i64, bool)> = tx
        .query(
            "SELECT item_id, revision, deleted_at IS NOT NULL
               FROM items WHERE vault_id = $1 AND item_id = ANY($2)",
            &[&session.vault_id, &ids],
        )
        .await?
        .iter()
        .map(|row| {
            (
                row.get::<_, Uuid>(0),
                (row.get::<_, i64>(1), row.get::<_, bool>(2)),
            )
        })
        .collect();

    let mut conflicts = Vec::new();
    for change in &req.changes {
        match stored.get(&change.item_id) {
            Some((revision, already_deleted)) => {
                if change.base_revision != Some(*revision) || (*already_deleted && change.deleted) {
                    conflicts.push(serde_json::json!({
                        "itemId": change.item_id,
                        "revision": revision,
                    }));
                }
            }
            None => {
                if change.base_revision.is_some() {
                    conflicts.push(serde_json::json!({
                        "itemId": change.item_id,
                        "revision": serde_json::Value::Null,
                    }));
                }
            }
        }
    }
    if !conflicts.is_empty() {
        // Nothing is written: the client pulls, sees what happened, and
        // decides. Dropping the transaction rolls it back.
        return Ok((
            StatusCode::CONFLICT,
            axum::Json(serde_json::json!({
                "error": error::body("conflict", "The stored revision has moved on.")["error"],
                "conflicts": conflicts,
            })),
        )
            .into_response());
    }

    let revision: i64 = tx
        .query_one(
            "UPDATE vaults SET revision = revision + 1 WHERE id = $1 RETURNING revision",
            &[&session.vault_id],
        )
        .await?
        .get(0);

    for change in &req.changes {
        if change.deleted {
            tx.execute(
                "INSERT INTO items (vault_id, item_id, overview, details, deleted_at, revision)
                 VALUES ($1, $2, NULL, NULL, now(), $3)
                 ON CONFLICT (vault_id, item_id) DO UPDATE
                   SET overview = NULL, details = NULL, deleted_at = now(), revision = $3",
                &[&session.vault_id, &change.item_id, &revision],
            )
            .await?;
        } else {
            let overview = &change.overview.as_ref().expect("checked above").0;
            let details = &change.details.as_ref().expect("checked above").0;
            tx.execute(
                "INSERT INTO items (vault_id, item_id, overview, details, deleted_at, revision)
                 VALUES ($1, $2, $3, $4, NULL, $5)
                 ON CONFLICT (vault_id, item_id) DO UPDATE
                   SET overview = $3, details = $4, deleted_at = NULL, revision = $5",
                &[
                    &session.vault_id,
                    &change.item_id,
                    overview,
                    details,
                    &revision,
                ],
            )
            .await?;
        }
    }
    tx.commit().await?;

    let applied: Vec<serde_json::Value> = req
        .changes
        .iter()
        .map(|c| serde_json::json!({ "itemId": c.item_id, "revision": revision }))
        .collect();
    tracing::info!(
        account_id = %session.account_id,
        changes = req.changes.len(),
        revision,
        "items written"
    );
    Ok(axum::Json(serde_json::json!({
        "cursor": revision,
        "applied": applied,
    }))
    .into_response())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FetchRequest {
    item_ids: Vec<Uuid>,
}

/// The current row for each requested item in the caller's vault, in the
/// pull's change shape. For a client retrying items it could not open. IDs
/// in another vault, or not at all, are simply absent: the answer is the
/// same either way, so it is not an oracle.
pub async fn fetch(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<FetchRequest>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    if req.item_ids.is_empty() || req.item_ids.len() > MAX_CHANGES_PER_BATCH {
        return Err(ApiError::InvalidRequest("itemIds is not valid"));
    }
    let distinct: HashSet<Uuid> = req.item_ids.iter().copied().collect();
    if distinct.len() != req.item_ids.len() {
        return Err(ApiError::InvalidRequest("an item appears twice"));
    }
    let db = state.pool.get().await?;
    let rows = db
        .query(
            "SELECT item_id, revision, overview, details, deleted_at IS NOT NULL
               FROM items WHERE vault_id = $1 AND item_id = ANY($2)
              ORDER BY item_id",
            &[&session.vault_id, &req.item_ids],
        )
        .await?;
    let changes: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let overview: Option<Vec<u8>> = row.get(2);
            let details: Option<Vec<u8>> = row.get(3);
            serde_json::json!({
                "itemId": row.get::<_, Uuid>(0),
                "revision": row.get::<_, i64>(1),
                "overview": overview.map(Blob),
                "details": details.map(Blob),
                "deleted": row.get::<_, bool>(4),
            })
        })
        .collect();
    Ok(axum::Json(serde_json::json!({ "changes": changes })))
}
