//! The only failures the API can express.
//!
//! Messages are fixed strings. None of them carries a value from the request,
//! so no email, token, blob or SQL fragment can reach a client or a log
//! through an error (CLAUDE.md §39).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiError {
    Unauthorized,
    NotFound,
    /// The stored revision moved on. Callers that need to name the conflicting
    /// items build their own 409 body instead of using this.
    Conflict,
    InvalidRequest(&'static str),
    TooLarge,
    /// The write would take the vault past `MAX_VAULT_BYTES` (SV-3).
    VaultFull,
    RateLimited,
    /// The token belonged to an account that was deleted (spec
    /// 2026-10-05-account-deletion §4.4). Only a holder of the token sees it.
    AccountDeleted,
    /// Every database connection or every key-check slot stayed busy for the
    /// whole wait (SV-4). Nothing happened; the client may retry.
    Unavailable,
    Internal,
}

impl ApiError {
    /// Status, code and message.
    pub(crate) fn parts(self) -> (StatusCode, &'static str, &'static str) {
        match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Authentication failed.",
            ),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found", "Not found."),
            Self::Conflict => (
                StatusCode::CONFLICT,
                "conflict",
                "The stored revision has moved on.",
            ),
            Self::InvalidRequest(m) => (StatusCode::BAD_REQUEST, "invalid_request", m),
            Self::TooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "too_large",
                "Request is too large.",
            ),
            Self::VaultFull => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "vault_full",
                "This vault has reached its storage limit.",
            ),
            Self::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Too many attempts. Try again later.",
            ),
            Self::AccountDeleted => (
                StatusCode::GONE,
                "account_deleted",
                "This account was deleted.",
            ),
            Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "The server is busy. Try again shortly.",
            ),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "Internal error.",
            ),
        }
    }
}

pub fn body(code: &str, message: &str) -> serde_json::Value {
    serde_json::json!({ "error": { "code": code, "message": message } })
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = self.parts();
        (status, Json(body(code, message))).into_response()
    }
}

/// A database failure is logged by kind and reported as `internal`. The
/// driver error never reaches the client: it can quote SQL, and SQL can
/// quote values.
impl From<tokio_postgres::Error> for ApiError {
    fn from(err: tokio_postgres::Error) -> Self {
        tracing::error!(kind = kind_of(&err), "database error");
        Self::Internal
    }
}

impl From<deadpool_postgres::PoolError> for ApiError {
    fn from(err: deadpool_postgres::PoolError) -> Self {
        if let deadpool_postgres::PoolError::Timeout(_) = err {
            tracing::warn!(kind = "pool_timeout", "database busy");
            return Self::Unavailable;
        }
        tracing::error!(kind = "pool", "database error");
        Self::Internal
    }
}

/// The error's own message can name a column or quote a value, so only the
/// SQLSTATE class reaches the log.
fn kind_of(err: &tokio_postgres::Error) -> &'static str {
    match err.code().map(|c| c.code().to_string()) {
        Some(code) if code.starts_with("23") => "integrity_violation",
        Some(code) if code.starts_with("08") => "connection",
        Some(code) if code.starts_with("40") => "serialization",
        Some(_) => "database",
        None => "client",
    }
}
