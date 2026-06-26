//! Health handler.
//!
//! | Method | Path     | Description           |
//! |--------|----------|-----------------------|
//! | `GET`  | `/health`| Liveness probe        |

use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::transform::TransformOperation;
use axum::extract::State;
use nvisy_core::service::health::{ComponentCheck, ServiceStatus};

use super::response::Health;
use crate::extract::Json;
use crate::middleware::{DEFAULT_HEALTH_TIMEOUT, RouterTimeoutExt};
use crate::service::ServiceState;

const TARGET: &str = "nvisy_server::infra";

#[tracing::instrument(target = TARGET, skip_all)]
async fn health_check(State(state): State<ServiceState>) -> Json<Health> {
    let fs_status = if state.data_dir().is_dir() {
        ServiceStatus::Healthy
    } else {
        ServiceStatus::Unhealthy
    };
    let checks = vec![ComponentCheck::new("filesystem", fs_status)];
    let overall = roll_up(&checks);
    if overall != ServiceStatus::Healthy {
        tracing::warn!(target: TARGET, ?overall, "health check degraded or unhealthy");
    }

    Json(Health {
        status: overall,
        checks,
        timestamp: jiff::Timestamp::now(),
    })
}

/// Worst-case roll-up: any `Unhealthy` wins; otherwise any
/// `Degraded` wins; otherwise `Healthy`.
fn roll_up(checks: &[ComponentCheck]) -> ServiceStatus {
    if checks.iter().any(|c| c.status == ServiceStatus::Unhealthy) {
        ServiceStatus::Unhealthy
    } else if checks.iter().any(|c| c.status == ServiceStatus::Degraded) {
        ServiceStatus::Degraded
    } else {
        ServiceStatus::Healthy
    }
}

fn health_docs(op: TransformOperation) -> TransformOperation {
    op.id("healthCheck")
        .tag("infra")
        .summary("Liveness probe")
        .description(
            "Checks that the server is running and the data directory is accessible. \
             Returns 200 with an overall status and per-component checks.",
        )
}

/// Health route (unversioned, served at `/health`).
pub fn health_routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route("/health", get_with(health_check, health_docs))
        .with_timeout(DEFAULT_HEALTH_TIMEOUT)
}
