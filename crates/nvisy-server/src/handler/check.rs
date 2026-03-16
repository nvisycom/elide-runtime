//! Health and analytics handlers.
//!
//! # Endpoints
//!
//! | Method | Path                  | Description                          |
//! |--------|-----------------------|--------------------------------------|
//! | `GET`  | `/health`             | Liveness probe (`{"status": "ok"}`)  |
//! | `GET`  | `/api/v1/analytics`   | Aggregate pipeline metrics           |

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::extract::State;
use nvisy_engine::{DefaultEngine, EngineAnalytics};

use super::response::{Analytics, Health, ServiceStatus};
use crate::extract::Json;
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::check";

/// `GET /health`: liveness probe.
#[tracing::instrument(target = "nvisy_server::check", skip_all)]
async fn health() -> Json<Health> {
    tracing::debug!(target: TARGET, "health check");
    Json(Health {
        status: ServiceStatus::Healthy,
        timestamp: jiff::Timestamp::now(),
    })
}

fn health_docs(op: TransformOperation) -> TransformOperation {
    op.id("healthCheck")
        .tag("infra")
        .summary("Liveness probe")
        .description("Returns 200 OK when the server is running.")
}

/// `GET /api/v1/analytics`: retrieve aggregate pipeline analytics.
#[tracing::instrument(target = "nvisy_server::check", skip_all)]
async fn analytics(State(engine): State<DefaultEngine>) -> Json<Analytics> {
    let snapshot = engine.snapshot().await;
    Json(Analytics {
        timestamp: snapshot.timestamp,
        total_runs: snapshot.total_runs,
        total_entities_detected: snapshot.total_entities_detected,
        total_redactions_applied: snapshot.total_redactions_applied,
    })
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
