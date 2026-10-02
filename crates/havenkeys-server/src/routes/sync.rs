//! Pull: everything that changed in this vault after a cursor.
//!
//! A change is an upsert or a deletion; there is nothing for the client to
//! decide, because the server is the only writer.
//!
//! The cursor is a revision, and every row a batch touched carries the same
//! one, so a page may only end where a revision ends. Cutting mid-revision
//! would hand out a cursor (`> revision`) that skips the rest of that batch,
//! and those items would never be pulled again. A page is cut by row count
//! and by bytes (`MAX_PULL_PAGE`, `MAX_PAGE_BYTES`), always at a revision
//! boundary and always holding at least one revision. A batch is capped at
//! `MAX_CHANGES_PER_BATCH` rows, so reading one page's worth plus one batch's
//! worth is always enough to find the boundary and still make progress.
//! Row sizes are read first; blobs are read only for the rows that go out.

use crate::auth::Session;
use crate::b64::Blob;
use crate::error::ApiError;
use crate::limits::{MAX_CHANGES_PER_BATCH, MAX_PAGE_BYTES, MAX_PULL_PAGE};
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

/// The JSON around one change's blobs: id, revision, flags, quotes.
pub(crate) const ROW_JSON_BYTES: usize = 256;

/// Length of `n` bytes as base64.
pub(crate) fn b64_len(n: i64) -> usize {
    usize::try_from(n).unwrap_or(0).div_ceil(3) * 4
}

/// How many of `rows` — `(revision, answer bytes)`, ordered by revision —
/// make one page: whole revisions only, at most `MAX_PULL_PAGE` rows and
/// `MAX_PAGE_BYTES` bytes, but always the first revision. When the query
/// window was full its last revision may be cut short, so it never ends a
/// page.
pub(crate) fn page_len(rows: &[(i64, usize)], window_full: bool) -> usize {
    let complete = match rows.last() {
        Some(&(last, _)) if window_full => rows.iter().position(|r| r.0 == last).unwrap_or(0),
        _ => rows.len(),
    };
    let (mut end, mut bytes) = (0usize, 0usize);
    while end < complete {
        let revision = rows[end].0;
        let next = end + rows[end..].iter().take_while(|r| r.0 == revision).count();
        let more: usize = rows[end..next].iter().map(|r| r.1).sum();
        if end > 0 && (next > MAX_PULL_PAGE as usize || bytes + more > MAX_PAGE_BYTES) {
            break;
        }
        end = next;
        bytes += more;
    }
    end
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
    // Sizes first: the blobs are read only for the rows that go out.
    let sized: Vec<(i64, usize)> = db
        .query(
            "SELECT revision,
                    COALESCE(octet_length(overview), 0)::bigint,
                    COALESCE(octet_length(details), 0)::bigint
               FROM items
              WHERE vault_id = $1 AND revision > $2
              ORDER BY revision, item_id
              LIMIT $3",
            &[&session.vault_id, &query.since, &window],
        )
        .await?
        .iter()
        .map(|r| {
            (
                r.get(0),
                b64_len(r.get(1)) + b64_len(r.get(2)) + ROW_JSON_BYTES,
            )
        })
        .collect();
    let end = page_len(&sized, sized.len() as i64 == window);
    let has_more = end < sized.len();

    // Rows rewritten since the size query moved past every revision here
    // and come in a later page; nothing can move into this range.
    let (changes, cursor) = match end.checked_sub(1).map(|last| sized[last].0) {
        Some(through) => {
            let rows = db
                .query(
                    "SELECT item_id, revision, overview, details, deleted_at IS NOT NULL
                       FROM items
                      WHERE vault_id = $1 AND revision > $2 AND revision <= $3
                      ORDER BY revision, item_id",
                    &[&session.vault_id, &query.since, &through],
                )
                .await?;
            (rows.iter().map(change_json).collect::<Vec<_>>(), through)
        }
        // With no rows left, the cursor jumps to the vault's own revision:
        // an idle client converges instead of asking for the same range
        // forever.
        None => (
            Vec::new(),
            db.query_one(
                "SELECT revision FROM vaults WHERE id = $1",
                &[&session.vault_id],
            )
            .await?
            .get(0),
        ),
    };

    Ok(axum::Json(serde_json::json!({
        "cursor": cursor,
        "hasMore": has_more,
        "changes": changes,
    })))
}

/// One change as a client reads it, from a row of
/// `item_id, revision, overview, details, deleted`.
pub(crate) fn change_json(row: &tokio_postgres::Row) -> serde_json::Value {
    let overview: Option<Vec<u8>> = row.get(2);
    let details: Option<Vec<u8>> = row.get(3);
    serde_json::json!({
        "itemId": row.get::<_, Uuid>(0),
        "revision": row.get::<_, i64>(1),
        "overview": overview.map(Blob),
        "details": details.map(Blob),
        "deleted": row.get::<_, bool>(4),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: usize = 1024 * 1024;
    const _: () = assert!(MAX_PAGE_BYTES >= 10 * MB && MAX_PAGE_BYTES < 15 * MB);
    const _: () = assert!(MAX_PULL_PAGE >= 400);

    #[test]
    fn b64_len_rounds_up_to_whole_quads() {
        assert_eq!(b64_len(0), 0);
        assert_eq!(b64_len(1), 4);
        assert_eq!(b64_len(3), 4);
        assert_eq!(b64_len(4), 8);
        assert_eq!(b64_len(-5), 0);
    }

    #[test]
    fn small_rows_page_by_count_at_revision_boundaries() {
        let mut rows: Vec<(i64, usize)> = (0..400).map(|_| (1, 100)).collect();
        rows.extend((0..400).map(|_| (2, 100)));
        // 800 rows, window not full: the first revision fits, the second
        // would pass MAX_PULL_PAGE, so the page ends after revision 1.
        assert_eq!(page_len(&rows, false), 400);
        let few = vec![(1, 100), (2, 100), (2, 100)];
        assert_eq!(page_len(&few, false), 3);
        assert_eq!(page_len(&[], false), 0);
    }

    #[test]
    fn big_rows_page_by_bytes() {
        let rows = vec![(1, 5 * MB), (2, 5 * MB), (3, 5 * MB)];
        // 10 MiB fits, 15 MiB does not.
        assert_eq!(page_len(&rows, false), 2);
    }

    #[test]
    fn page_len_always_takes_the_first_revision() {
        let rows = vec![(1, 15 * MB), (2, 1)];
        assert_eq!(page_len(&rows, false), 1);
    }

    #[test]
    fn a_full_window_never_ends_on_its_last_revision() {
        // The window may have cut revision 2 short: it is left for the
        // next page even though it would fit.
        let rows = vec![(1, 10), (2, 10), (2, 10)];
        assert_eq!(page_len(&rows, true), 1);
    }
}
