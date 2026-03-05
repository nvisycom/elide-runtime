//! Health and analytics handlers.
//!
//! # Endpoints
//!
//! | Method | Path                  | Description                          |
//! |--------|-----------------------|--------------------------------------|
//! | `GET`  | `/health`             | Liveness probe (`{"status": "ok"}`)  |
//! | `GET`  | `/api/v1/analytics`   | Aggregate pipeline metrics (stub)    |

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;

use super::error::{ErrorKind, Result};
use super::response::{Analytics, Health};
use crate::extract::Json;
use crate::service::ServiceState;

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
async fn analytics() -> Result<Json<Analytics>> {
    Err(ErrorKind::NotImplemented.with_message("analytics endpoint not yet implemented"))
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
