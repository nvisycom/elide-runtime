use aide::axum::IntoApiResponse;
use axum::Json;
use schemars::JsonSchema;
use serde::Serialize;

/// Health check response.
#[derive(Serialize, JsonSchema)]
pub struct HealthResponse {
    pub status: String,
}

/// `GET /health` — liveness probe.
pub async fn health() -> impl IntoApiResponse {
    Json(HealthResponse {
        status: "ok".into(),
    })
}
