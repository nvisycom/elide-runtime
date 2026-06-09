//! Health handler.
//!
//! | Method | Path          | Description                          |
//! |--------|---------------|--------------------------------------|
//! | `GET`  | `/health`     | Liveness probe (unversioned)         |

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::extract::State;
use nvisy_engine::pipeline::Engine;

use super::response::{ComponentCheck, Health, ServiceStatus};
use crate::extract::Json;
use crate::middleware::{DEFAULT_HEALTH_TIMEOUT, RouterTimeoutExt};
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::infra";

/// `GET /health`
#[tracing::instrument(target = TARGET, skip_all)]
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

    let registry_ok = engine.registry().healthcheck().await.is_ok();
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

/// Health route (unversioned, served at `/health`).
pub fn health_routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route("/health", get_with(health_check, health_docs))
        .with_timeout(DEFAULT_HEALTH_TIMEOUT)
}
