use axum::{Router, routing::get, Json};
use crate::service::AppState;

/// Health check response.
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy")
    )
)]
async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

/// Readiness check response.
#[utoipa::path(
    get,
    path = "/ready",
    responses(
        (status = 200, description = "Service is ready")
    )
)]
async fn ready() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ready" }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
}
