//! Pull: everything that changed in this vault after a cursor.
//!
//! A change is an upsert or a deletion; there is nothing for the client to
//! decide, because the server is the only writer.
//!
//! The cursor is a revision, and every row a batch touched carries the same
//! one, so a page may only end where a revision ends. Cutting mid-revision
//! would hand out a cursor (`> revision`) that skips the rest of that batch,
//! and those items would never be pulled again. A batch is capped at
//! `MAX_CHANGES_PER_BATCH` rows, so reading one page's worth plus one batch's
//! worth is always enough to find the boundary and still make progress.

use crate::auth::Session;
use crate::b64::Blob;
use crate::error::ApiError;
use crate::limits::{MAX_CHANGES_PER_BATCH, MAX_PULL_PAGE};
use crate::routes::AppState;
use axum::extract::{Query, State};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PullQuery {
    #[serde(default)]
    since: i64,
}

pub async fn pull(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<PullQuery>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    if query.since < 0 {
        return Err(ApiError::InvalidRequest("since is not valid"));
    }
    let db = state.pool.get().await?;
    let window = MAX_PULL_PAGE + MAX_CHANGES_PER_BATCH as i64 + 1;
    let rows = db
        .query(
            "SELECT item_id, revision, overview, details, deleted_at IS NOT NULL
               FROM items
              WHERE vault_id = $1 AND revision > $2
              ORDER BY revision, item_id
              LIMIT $3",
            &[&session.vault_id, &query.since, &window],
        )
        .await?;

    let page = trim_to_revision_boundary(&rows);
    let has_more = page.len() < rows.len();
    let changes: Vec<serde_json::Value> = page
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

    // With no rows left, the cursor jumps to the vault's own revision: an
    // idle client converges instead of asking for the same range forever.
    let cursor = match page.last() {
        Some(row) => row.get::<_, i64>(1),
        None => {
            db.query_one(
                "SELECT revision FROM vaults WHERE id = $1",
                &[&session.vault_id],
            )
            .await?
            .get(0)
        }
    };

    Ok(axum::Json(serde_json::json!({
        "cursor": cursor,
        "hasMore": has_more,
        "changes": changes,
    })))
}

/// Cut the fetched rows at the last complete revision that fits in a page.
///
/// Rows arrive ordered by `(revision, item_id)`. If they fit in a page, all
/// of them go out. Otherwise the first row that would not fit names the
/// revision to stop before — and because one revision holds at most one
/// batch's rows, and the window is a page plus a batch, at least one complete
/// revision always fits.
fn trim_to_revision_boundary(rows: &[tokio_postgres::Row]) -> &[tokio_postgres::Row] {
    if rows.len() <= MAX_PULL_PAGE as usize {
        return rows;
    }
    let boundary: i64 = rows[MAX_PULL_PAGE as usize].get(1);
    let end = rows
        .iter()
        .position(|row| row.get::<_, i64>(1) >= boundary)
        .unwrap_or(rows.len());
    &rows[..end]
}
