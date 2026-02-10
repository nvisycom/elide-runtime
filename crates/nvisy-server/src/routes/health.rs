use axum::{Router, routing::get, Json};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn ready() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ready" }))
}
