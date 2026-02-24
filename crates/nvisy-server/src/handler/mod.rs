mod analytics;
mod content;
mod execute;
mod redaction;

pub mod request;
pub mod response;

use aide::axum::ApiRouter;
use aide::axum::routing::{get, post};
use aide::scalar::Scalar;

use crate::service::ServiceState;

/// Build the handler route tree.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .api_route("/api/v1/analytics", get(analytics::summary))
        .api_route("/api/v1/execute", post(execute::execute))
        .route("/api/v1/content", axum::routing::post(content::upload))
        .api_route("/api/v1/content/{id}", get(content::download))
        .api_route("/api/v1/redaction", post(redaction::redact))
        .route(
            "/api/v1/openapi.json",
            axum::routing::get(execute::openapi_json),
        )
        .route("/docs", Scalar::new("/api/v1/openapi.json").axum_route())
}
