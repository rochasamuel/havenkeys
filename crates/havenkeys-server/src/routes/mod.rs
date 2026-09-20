//! Router assembly and the state every handler shares.

use crate::limits::MAX_BODY_BYTES;
use axum::routing::get;
use axum::Router;
use deadpool_postgres::Pool;
use tower_http::limit::RequestBodyLimitLayer;

pub mod health;

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
        // Checked before the body is read, so an oversized request never
        // reaches serde and never allocates.
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        .with_state(state)
}
