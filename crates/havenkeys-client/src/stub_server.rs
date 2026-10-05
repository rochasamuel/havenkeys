//! A stand-in for the server's sign-in routes, for tests that must hold a
//! login open while the vault locks. Only the answers `SyncClient` needs to
//! succeed are given; nothing is checked.
//!
//! It also keeps items in memory (pull, write, fetch) with the real server's
//! revision and tombstone rules (`havenkeys-server/src/routes/items.rs`):
//! one vault revision bumped per accepted batch and stamped on every row it
//! touches; a deletion keeps the row as a tombstone; a change whose base
//! revision is not the stored one (or a create for an ID the server has
//! seen, deleted or not) fails the whole batch with 409 naming each
//! conflicting item and its stored revision. Blobs are opaque here, so it
//! does not check sizes or sessions.

use axum::routing::{get, post};
use axum::Router;
use havenkeys_core::crypto::kdf::KdfParams;
use havenkeys_sync_client::wire::KdfDto;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
use uuid::Uuid;

/// What the stub tells a device about its account.
pub(crate) struct StubAccount {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub kdf: KdfParams,
    /// The attested header, as `encode_header_for` builds it.
    pub header: Vec<u8>,
    pub header_revision: i64,
}

/// One stored item: base64 blobs, or `None` for a tombstone.
#[derive(Clone)]
pub(crate) struct StubRow {
    pub revision: i64,
    pub blobs: Option<(String, String)>,
}

#[derive(Default)]
pub(crate) struct StubItems {
    /// The vault revision: the cursor of the latest accepted batch.
    pub revision: i64,
    pub rows: BTreeMap<Uuid, StubRow>,
}

impl StubItems {
    fn change_json(id: &Uuid, row: &StubRow) -> serde_json::Value {
        serde_json::json!({
            "itemId": id,
            "revision": row.revision,
            "overview": row.blobs.as_ref().map(|b| b.0.clone()),
            "details": row.blobs.as_ref().map(|b| b.1.clone()),
            "deleted": row.blobs.is_none(),
        })
    }

    fn pull(&self, since: i64) -> String {
        let mut rows: Vec<_> = self
            .rows
            .iter()
            .filter(|(_, r)| r.revision > since)
            .collect();
        rows.sort_by_key(|(id, r)| (r.revision, **id));
        let cursor = rows.last().map_or(since, |(_, r)| r.revision);
        let changes: Vec<_> = rows
            .iter()
            .map(|(id, r)| Self::change_json(id, r))
            .collect();
        serde_json::json!({ "cursor": cursor, "hasMore": false, "changes": changes }).to_string()
    }

    /// `routes/items.rs::write`: all or nothing.
    fn write(&mut self, body: &str) -> (axum::http::StatusCode, String) {
        let req: serde_json::Value = serde_json::from_str(body).unwrap();
        let changes = req["changes"].as_array().unwrap();
        let mut conflicts = Vec::new();
        for c in changes {
            let id: Uuid = c["itemId"].as_str().unwrap().parse().unwrap();
            let base = c["baseRevision"].as_i64();
            let deleted = c["deleted"].as_bool().unwrap_or(false);
            match self.rows.get(&id) {
                // `find_conflicts`, stored row.
                Some(row) => {
                    if base != Some(row.revision) || (row.blobs.is_none() && deleted) {
                        conflicts
                            .push(serde_json::json!({ "itemId": id, "revision": row.revision }));
                    }
                }
                None => {
                    if base.is_some() {
                        conflicts.push(serde_json::json!({ "itemId": id, "revision": null }));
                    }
                }
            }
        }
        if !conflicts.is_empty() {
            let body = serde_json::json!({
                "error": { "code": "conflict", "message": "conflict" },
                "conflicts": conflicts,
            });
            return (axum::http::StatusCode::CONFLICT, body.to_string());
        }
        self.revision += 1;
        let revision = self.revision;
        let mut applied = Vec::new();
        for c in changes {
            let id: Uuid = c["itemId"].as_str().unwrap().parse().unwrap();
            let blobs = if c["deleted"].as_bool().unwrap_or(false) {
                None
            } else {
                Some((
                    c["overview"].as_str().unwrap().to_owned(),
                    c["details"].as_str().unwrap().to_owned(),
                ))
            };
            self.rows.insert(id, StubRow { revision, blobs });
            applied.push(serde_json::json!({ "itemId": id, "revision": revision }));
        }
        let body = serde_json::json!({ "cursor": revision, "applied": applied });
        (axum::http::StatusCode::OK, body.to_string())
    }

    fn fetch(&self, body: &str) -> String {
        let req: serde_json::Value = serde_json::from_str(body).unwrap();
        let changes: Vec<_> = req["itemIds"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| {
                let id: Uuid = v.as_str()?.parse().ok()?;
                self.rows.get(&id).map(|r| Self::change_json(&id, r))
            })
            .collect();
        serde_json::json!({ "changes": changes, "unanswered": [] }).to_string()
    }
}

pub(crate) struct StubServer {
    /// The server's items, for tests to inspect or change behind the
    /// client's back.
    pub items: Arc<Mutex<StubItems>>,
    pub url: String,
    /// Notified when a login request arrives.
    pub login_entered: Arc<Notify>,
    /// The login answers only once this is notified. A permit given before
    /// the request arrives lets it straight through.
    pub release_login: Arc<Notify>,
    /// While true, login answers 401 at once.
    pub refuse_login: Arc<std::sync::atomic::AtomicBool>,
}

impl StubServer {
    pub async fn start(account: StubAccount) -> Self {
        let login_entered = Arc::new(Notify::new());
        let release_login = Arc::new(Notify::new());
        let params = serde_json::json!({
            "accountId": account.account_id,
            "kdf": KdfDto::from(&account.kdf),
        })
        .to_string();
        let login = serde_json::json!({
            "token": "stub-token",
            "expiresAt": "2099-01-01T00:00:00Z",
            "vaultId": account.vault_id,
        })
        .to_string();
        let header = serde_json::json!({
            "header": data_encoding::BASE64.encode(&account.header),
            "headerRevision": account.header_revision,
            "keyScheme": havenkeys_sync_client::client::KEY_SCHEME,
        })
        .to_string();

        let items = Arc::new(Mutex::new(StubItems::default()));
        let (for_pull, for_write, for_fetch) = (items.clone(), items.clone(), items.clone());
        let refuse = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let refuse_for_route = refuse.clone();
        let (entered, release) = (login_entered.clone(), release_login.clone());
        let app = Router::new()
            .route("/v1/auth/params", post(move || async move { params }))
            .route(
                "/v1/auth/login",
                post(move || async move {
                    use axum::response::IntoResponse;
                    if refuse_for_route.load(std::sync::atomic::Ordering::SeqCst) {
                        return axum::http::StatusCode::UNAUTHORIZED.into_response();
                    }
                    entered.notify_one();
                    release.notified().await;
                    login.into_response()
                }),
            )
            .route("/v1/vault/header", get(move || async move { header }))
            .route(
                "/v1/sync",
                get(move |uri: axum::http::Uri| async move {
                    let since = uri
                        .query()
                        .and_then(|q| q.strip_prefix("since="))
                        .and_then(|n| n.parse().ok())
                        .unwrap_or(0);
                    for_pull.lock().unwrap().pull(since)
                }),
            )
            .route(
                "/v1/items",
                post(move |body: String| async move {
                    use axum::response::IntoResponse;
                    for_write.lock().unwrap().write(&body).into_response()
                }),
            )
            .route(
                "/v1/items/fetch",
                post(move |body: String| async move { for_fetch.lock().unwrap().fetch(&body) }),
            );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Self {
            items,
            url: format!("http://{addr}"),
            login_entered,
            release_login,
            refuse_login: refuse,
        }
    }
}
