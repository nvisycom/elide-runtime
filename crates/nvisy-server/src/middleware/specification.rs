//! OpenAPI specification middleware with Scalar UI integration.
//!
//! Provides OpenAPI spec generation from aide's [`ApiRouter`] and serves
//! the spec JSON alongside a Scalar interactive API reference.
//!
//! # Usage
//!
//! ```rust,ignore
//! use aide::axum::ApiRouter;
//! use nvisy_server::middleware::specification::{OpenApiConfig, RouterOpenApiExt};
//!
//! let app = ApiRouter::new()
//!     .with_open_api(&OpenApiConfig::default());
//! ```
//!
//! [`ApiRouter`]: aide::axum::ApiRouter

use aide::axum::ApiRouter;
use aide::openapi::{OpenApi, Tag};
use aide::scalar::Scalar;
use aide::transform::TransformOpenApi;
use axum::routing::get;
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};

/// OpenAPI configuration for aide integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenApiConfig {
    /// API title shown in the OpenAPI spec.
    pub title: String,
    /// API version shown in the OpenAPI spec.
    pub version: String,
    /// API description shown in the OpenAPI spec.
    pub description: Option<String>,
    /// Path that exposes the OpenAPI JSON specification.
    pub spec_path: String,
    /// Path that exposes the Scalar API reference UI.
    pub ui_path: String,
}

impl Default for OpenApiConfig {
    fn default() -> Self {
        Self {
            title: "nvisy API".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            description: Some("REST API for the nvisy redaction engine.".to_owned()),
            spec_path: "/api/v1/openapi.json".to_owned(),
            ui_path: "/docs".to_owned(),
        }
    }
}

/// Extension trait for [`ApiRouter`] to add OpenAPI documentation with Scalar UI.
///
/// This consumes the `ApiRouter<S>` and returns a `Router<S>` because
/// `finish_api_with` finalizes the aide route tree.
pub trait RouterOpenApiExt<S> {
    /// Generates the OpenAPI specification, adds the JSON and Scalar UI routes,
    /// and injects the spec as an `Extension`.
    fn with_open_api(self, config: &OpenApiConfig) -> Router<S>;
}

impl<S> RouterOpenApiExt<S> for ApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_open_api(self, config: &OpenApiConfig) -> Router<S> {
        let mut api = OpenApi::default();

        let router = self
            .route(&config.spec_path, get(serve_spec))
            .route(&config.ui_path, Scalar::new(&config.spec_path).axum_route());

        let info = config.clone();
        router
            .finish_api_with(&mut api, |t| api_docs(t, info))
            .layer(Extension(api))
    }
}

/// `GET /api/v1/openapi.json`: serve the generated OpenAPI spec.
async fn serve_spec(Extension(api): Extension<OpenApi>) -> Json<OpenApi> {
    Json(api)
}

/// Populate the OpenAPI spec with info and tags.
///
/// Takes ownership of `config` so it can set string fields on the spec
/// without lifetime issues from the `finish_api_with` closure.
fn api_docs(mut api: TransformOpenApi<'_>, config: OpenApiConfig) -> TransformOpenApi<'_> {
    api.inner_mut().info = aide::openapi::Info {
        title: config.title,
        version: config.version,
        description: config.description,
        ..Default::default()
    };

    api.tag(Tag {
        name: "infra".into(),
        description: Some("Health checks and analytics".into()),
        ..Default::default()
    })
    .tag(Tag {
        name: "runs".into(),
        description: Some("Pipeline execution, inspection, and cancellation".into()),
        ..Default::default()
    })
    .tag(Tag {
        name: "files".into(),
        description: Some("Content file upload, download, and management".into()),
        ..Default::default()
    })
    .tag(Tag {
        name: "contexts".into(),
        description: Some("Reference-data context upload and management".into()),
        ..Default::default()
    })
}
