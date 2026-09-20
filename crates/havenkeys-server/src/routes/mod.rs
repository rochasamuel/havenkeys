//! Router assembly and the state every handler shares.

use crate::limits::MAX_BODY_BYTES;
use axum::routing::{get, post};
use axum::Router;
use deadpool_postgres::Pool;
use tower_http::limit::RequestBodyLimitLayer;

pub mod accounts;
pub mod auth;
pub mod health;
pub mod items;
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
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health::health))
        .route("/v1/accounts/activate", post(accounts::activate))
        .route("/v1/auth/params", post(auth::params))
        .route("/v1/auth/login", post(auth::login))
        .route("/v1/auth/logout", post(auth::logout))
        .route(
            "/v1/vault/header",
            get(vault::get_header).put(vault::put_header),
        )
        .route("/v1/sync", get(sync::pull))
        .route("/v1/items", post(items::write))
        // Checked before the body is read, so an oversized request never
        // reaches serde and never allocates.
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        .with_state(state)
}
