//! Health and analytics handlers.
//!
//! - `GET /health` — liveness probe returning `{"status": "ok"}`.
//! - `GET /api/v1/analytics` — aggregate pipeline metrics (stub).

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::Json;
use nvisy_core::{Error, ErrorKind};
use schemars::JsonSchema;
use serde::Serialize;

use super::response::{Analytics, ServerError};
use crate::service::ServiceState;

/// Response body for `GET /health`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Health {
    pub status: &'static str,
}

/// `GET /health`: liveness probe.
async fn health() -> Json<Health> {
    Json(Health { status: "ok" })
}

fn health_docs(op: TransformOperation) -> TransformOperation {
    op.id("healthCheck")
        .tag("infra")
        .summary("Liveness probe")
        .description("Returns 200 OK when the server is running.")
}

/// `GET /api/v1/analytics`: retrieve aggregate pipeline analytics.
#[tracing::instrument(skip_all)]
async fn analytics(
    State(_state): State<ServiceState>,
) -> Result<Json<Analytics>, ServerError> {
    Err(ServerError::from(Error::new(
        ErrorKind::Runtime,
        "analytics endpoint not yet implemented",
    )))
}

fn analytics_docs(op: TransformOperation) -> TransformOperation {
    op.id("getAnalytics")
        .tag("infra")
        .summary("Retrieve aggregate pipeline analytics")
        .description("Returns aggregate metrics across all pipeline runs.")
}

/// Check routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route("/health", get_with(health, health_docs))
        .api_route("/api/v1/analytics", get_with(analytics, analytics_docs))
}
