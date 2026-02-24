mod analytics;
mod content;
mod error;
mod execute;
mod health;
mod redaction;

use aide::axum::ApiRouter;
use aide::axum::routing::{get, post};
use aide::scalar::Scalar;

use crate::service::ServiceState;

/// Build the handler route tree.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route("/health", get(health::health))
        .api_route("/api/v1/execute", post(execute::execute))
        .api_route("/api/v1/content", post(content::upload))
        .api_route("/api/v1/content/{id}", get(content::download))
        .api_route("/api/v1/redaction", post(redaction::redact))
        .api_route("/api/v1/analytics", get(analytics::summary))
        .route(
            "/api/v1/openapi.json",
            axum::routing::get(execute::openapi_json),
        )
        .route("/docs", Scalar::new("/api/v1/openapi.json").axum_route())
}
