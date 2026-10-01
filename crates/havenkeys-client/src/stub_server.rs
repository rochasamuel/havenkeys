//! A stand-in for the server's sign-in routes, for tests that must hold a
//! login open while the vault locks. Only the answers `SyncClient` needs to
//! succeed are given; nothing is checked.

use axum::routing::{get, post};
use axum::Router;
use havenkeys_core::crypto::kdf::KdfParams;
use havenkeys_sync_client::wire::KdfDto;
use std::sync::Arc;
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

pub(crate) struct StubServer {
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
            .route("/v1/vault/header", get(move || async move { header }));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Self {
            url: format!("http://{addr}"),
            login_entered,
            release_login,
            refuse_login: refuse,
        }
    }
}
