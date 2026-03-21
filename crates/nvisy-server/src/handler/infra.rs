//! Health and analytics handlers.
//!
//! # Endpoints
//!
//! | Method | Path                  | Description                          |
//! |--------|-----------------------|--------------------------------------|
//! | `GET`  | `/health`             | Liveness probe                       |
//! | `GET`  | `/api/v1/analytics`   | Aggregate pipeline metrics           |

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::extract::State;
use nvisy_engine::pipeline::{DefaultEngine, EngineAnalytics};

use super::response::{Analytics, Health, ServiceStatus};
use crate::extract::Json;
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::infra";

/// `GET /health`: liveness probe.
///
/// Verifies the data directory is accessible and returns the service status.
#[tracing::instrument(target = "nvisy_server::infra", skip_all)]
async fn health_check(State(engine): State<DefaultEngine>) -> Json<Health> {
    let status = if engine.registry().base_dir().is_dir() {
        ServiceStatus::Healthy
    } else {
        tracing::warn!(target: TARGET, "data directory is not accessible");
        ServiceStatus::Unhealthy
    };

    tracing::debug!(target: TARGET, ?status, "health check");
    Json(Health {
        status,
        timestamp: jiff::Timestamp::now(),
    })
}

fn health_docs(op: TransformOperation) -> TransformOperation {
    op.id("healthCheck")
        .tag("infra")
        .summary("Liveness probe")
        .description(
            "Checks that the server is running and the data directory is accessible. \
             Returns 200 with status `healthy` or `unhealthy`.",
        )
}

/// `GET /api/v1/analytics`: retrieve aggregate pipeline analytics.
#[tracing::instrument(target = "nvisy_server::infra", skip_all)]
async fn get_analytics(State(engine): State<DefaultEngine>) -> Json<Analytics> {
    let snapshot = engine.snapshot().await;
    Json(Analytics {
        timestamp: snapshot.timestamp,
        total_runs: snapshot.total_runs,
        active_runs: snapshot.active_runs,
        succeeded_runs: snapshot.succeeded_runs,
        failed_runs: snapshot.failed_runs,
        cancelled_runs: snapshot.cancelled_runs,
        total_entities_detected: snapshot.total_entities_detected,
        total_redactions_applied: snapshot.total_redactions_applied,
        distinct_actors: snapshot.distinct_actors,
    })
}

fn analytics_docs(op: TransformOperation) -> TransformOperation {
    op.id("getAnalytics")
        .tag("infra")
        .summary("Retrieve aggregate pipeline analytics")
        .description("Returns aggregate metrics across all pipeline runs.")
}

/// Infra routes.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route("/health", get_with(health_check, health_docs))
        .api_route("/api/v1/analytics", get_with(get_analytics, analytics_docs))
}
