//! The platform health check. Unauthenticated on purpose, and it says nothing
//! about any vault: only that the process is up and the database answers.

use crate::routes::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

pub async fn health(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let reachable = match state.pool.get().await {
        Ok(client) => client.query_one("SELECT 1", &[]).await.is_ok(),
        Err(_) => false,
    };
    if reachable {
        (StatusCode::OK, Json(serde_json::json!({"status": "ok"})))
    } else {
        tracing::error!("health check could not reach the database");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"status": "unavailable"})),
        )
    }
}
