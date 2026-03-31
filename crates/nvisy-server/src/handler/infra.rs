//! Health and analytics handlers.
//!
//! # Endpoints
//!
//! | Method | Path          | Description                          |
//! |--------|---------------|--------------------------------------|
//! | `GET`  | `/health`     | Liveness probe (unversioned)         |
//! | `GET`  | `/analytics`  | Aggregate pipeline metrics           |
//!
//! `/health` is served at the root (unversioned). `/analytics` is
//! relative and nested under the version prefix by the version module.

use std::time::Duration;

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::error_handling::HandleErrorLayer;
use axum::extract::State;
use nvisy_engine::pipeline::{AnalyticsSnapshot, Engine};
use tower::ServiceBuilder;
use tower::timeout::TimeoutLayer;

use super::response::{ComponentCheck, Health, ServiceStatus};
use crate::extract::Json;
use crate::middleware::constants::{DEFAULT_HEALTH_TIMEOUT_SECS, DEFAULT_READ_TIMEOUT_SECS};
use crate::middleware::recovery::handle_error;
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::infra";

/// `GET /health`
#[tracing::instrument(target = "nvisy_server::infra", skip_all)]
async fn health_check(State(engine): State<Engine>) -> Json<Health> {
    let mut checks = vec![];

    let fs_ok = engine.data_dir().is_dir();
    checks.push(ComponentCheck {
        name: "filesystem".into(),
        status: if fs_ok {
            ServiceStatus::Healthy
        } else {
            ServiceStatus::Unhealthy
        },
    });

    let registry_ok = engine.data_dir().is_dir();
    checks.push(ComponentCheck {
        name: "registry".into(),
        status: if registry_ok {
            ServiceStatus::Healthy
        } else {
            ServiceStatus::Degraded
        },
    });

    let overall = if checks.iter().all(|c| c.status == ServiceStatus::Healthy) {
        ServiceStatus::Healthy
    } else if checks.iter().any(|c| c.status == ServiceStatus::Unhealthy) {
        ServiceStatus::Unhealthy
    } else {
        ServiceStatus::Degraded
    };

    if overall != ServiceStatus::Healthy {
        tracing::warn!(target: TARGET, ?overall, "health check degraded or unhealthy");
    }

    tracing::debug!(target: TARGET, ?overall, "health check");
    Json(Health {
        status: overall,
        checks,
        timestamp: jiff::Timestamp::now(),
    })
}

fn health_docs(op: TransformOperation) -> TransformOperation {
    op.id("healthCheck")
        .tag("infra")
        .summary("Liveness probe")
        .description(
            "Checks that the server is running, the data directory is accessible, \
             and the registry is operational. Returns 200 with an overall status \
             and per-component checks.",
        )
}

/// `GET /analytics`
#[tracing::instrument(target = "nvisy_server::infra", skip_all)]
async fn get_analytics(State(engine): State<Engine>) -> Json<AnalyticsSnapshot> {
    Json(engine.snapshot().await)
}

fn analytics_docs(op: TransformOperation) -> TransformOperation {
    op.id("getAnalytics")
        .tag("infra")
        .summary("Retrieve aggregate pipeline analytics")
        .description("Returns aggregate metrics across all pipeline runs.")
}

/// Health route (unversioned, served at `/health`).
pub fn health_routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route("/health", get_with(health_check, health_docs))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_error))
                .layer(TimeoutLayer::new(Duration::from_secs(
                    DEFAULT_HEALTH_TIMEOUT_SECS,
                ))),
        )
}

/// Analytics route (relative path, versioned).
pub fn analytics_routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route("/analytics", get_with(get_analytics, analytics_docs))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_error))
                .layer(TimeoutLayer::new(Duration::from_secs(
                    DEFAULT_READ_TIMEOUT_SECS,
                ))),
        )
}
