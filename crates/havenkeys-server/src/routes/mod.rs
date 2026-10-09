//! Router assembly and the state every handler shares.

use crate::limits::{
    MAX_ANONYMOUS_AUTH_BODY_BYTES, MAX_ANONYMOUS_PAIRING_BODY_BYTES, MAX_BODY_BYTES,
};
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderName, Method, Request};
use axum::routing::{delete, get, post, MethodRouter};
use axum::Router;
use deadpool_postgres::Pool;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

pub mod account;
pub mod accounts;
pub mod auth;
pub mod devices;
pub mod health;
pub mod items;
pub mod pairings;
pub mod signup;
pub mod sync;
pub mod vault;

#[derive(Clone)]
pub struct AppState {
    pub pool: Pool,
    /// Used only to derive the decoy account id and salt that make
    /// `auth/params` answer uniformly for an unknown email.
    pub server_secret: [u8; 32],
    /// Whether the left-most `X-Forwarded-For` entry may be believed. Off
    /// unless the deployment is known to sit behind a proxy that sets it.
    pub trust_forwarded_for: bool,
    /// Exactly one browser origin, or none. There is no wildcard: the desktop
    /// app is not a browser, so the default is that no web page may call this
    /// API at all (design §7.6).
    pub cors_origin: Option<String>,
    /// Turns a client address into a coarse place name for the pairing
    /// prompt; none when no location database is configured.
    pub locator: Option<std::sync::Arc<crate::locate::Locator>>,
    /// The most blob bytes one vault may hold: `limits::MAX_VAULT_BYTES`,
    /// smaller in tests.
    pub max_vault_bytes: i64,
    /// Sends the signup and trial emails; none when SMTP is not configured.
    pub mailer: Option<std::sync::Arc<dyn crate::mail::Mailer>>,
    /// The server's public URL when signup is open (`HAVENKEYS_SIGNUP=open`),
    /// carried in the invites it issues; `None` answers the signup routes
    /// with 404.
    pub signup_url: Option<String>,
}

/// A route anyone may call, without a session: its own, much smaller body
/// limit, inside the router-wide one (PA6).
fn small(route: MethodRouter<AppState>, max: usize) -> MethodRouter<AppState> {
    route
        .layer::<_, std::convert::Infallible>(RequestBodyLimitLayer::new(max))
        .layer(DefaultBodyLimit::max(max))
}

pub fn router(state: AppState) -> Router {
    let cors = state.cors_origin.clone().and_then(build_cors);
    let router = Router::new()
        .route("/v1/health", get(health::health))
        .route("/v1/accounts/activate", post(accounts::activate))
        .route(
            "/v1/signup/start",
            small(post(signup::start), MAX_ANONYMOUS_AUTH_BODY_BYTES),
        )
        .route(
            "/v1/signup/verify",
            small(post(signup::verify), MAX_ANONYMOUS_AUTH_BODY_BYTES),
        )
        .route("/v1/account/credentials", post(account::change_credentials))
        .route("/v1/account/delete", post(account::delete_account))
        .route(
            "/v1/auth/params",
            small(post(auth::params), MAX_ANONYMOUS_AUTH_BODY_BYTES),
        )
        .route(
            "/v1/auth/login",
            small(post(auth::login), MAX_ANONYMOUS_AUTH_BODY_BYTES),
        )
        .route("/v1/auth/logout", post(auth::logout))
        .route("/v1/vault/header", get(vault::get_header))
        .route("/v1/sync", get(sync::pull))
        .route("/v1/items", post(items::write))
        .route("/v1/items/fetch", post(items::fetch))
        .route("/v1/devices", get(devices::list))
        .route("/v1/devices/{id}", delete(devices::revoke))
        .route(
            "/v1/pairings",
            small(post(pairings::create), MAX_ANONYMOUS_PAIRING_BODY_BYTES),
        )
        .route("/v1/pairings/{id}", get(pairings::details))
        .route("/v1/pairings/{id}/approve", post(pairings::approve))
        .route("/v1/pairings/{id}/deny", post(pairings::deny))
        .route(
            "/v1/pairings/{id}/claim",
            small(post(pairings::claim), MAX_ANONYMOUS_PAIRING_BODY_BYTES),
        )
        // Checked before the body is read, so an oversized request never
        // reaches serde and never allocates. Both layers are needed: the
        // tower layer stops a declared oversize immediately, and axum's own
        // limit (2 MiB by default, which is below a legitimate batch of
        // blobs) is what the extractors enforce while streaming.
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        // Method, path, status and latency only. Never the query string,
        // never headers, never a body: all three can carry secrets.
        .layer(
            TraceLayer::new_for_http().make_span_with(|req: &Request<_>| {
                tracing::info_span!(
                    "request",
                    method = %req.method(),
                    path = req.uri().path(),
                )
            }),
        );
    let router = match cors {
        Some(layer) => router.layer(layer),
        None => router,
    };
    router.with_state(state)
}

/// One exact origin, or nothing. A malformed value is dropped rather than
/// widened, so a typo cannot turn into an open API.
fn build_cors(origin: String) -> Option<CorsLayer> {
    let value = origin.parse().ok()?;
    Some(
        CorsLayer::new()
            .allow_origin(AllowOrigin::exact(value))
            .allow_methods([Method::GET, Method::POST, Method::DELETE])
            .allow_headers([
                HeaderName::from_static("authorization"),
                HeaderName::from_static("content-type"),
            ]),
    )
}
