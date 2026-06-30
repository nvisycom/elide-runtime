//! HTTP handler functions and route wiring.
//!
//! Each submodule corresponds to an API resource and exposes a
//! `routes_v1()` function that returns relative-path `ApiRouter`
//! fragments. These are nested under `/api/v1` by [`routes`].
//!
//! The top-level [`routes()`] function assembles all versions
//! plus unversioned routes (health, docs) into a single router.
//!
//! Request and response types live in the `request` and
//! `response` submodules. [`Error`], [`ErrorKind`], and
//! [`Result`] are re-exported for use by middleware and
//! extractors.

pub mod error;

mod contexts;
mod detections;
mod files;
mod infra;
mod policies;
mod redactions;

pub mod request;
pub mod response;

use aide::axum::ApiRouter;
use axum::http::Uri;

pub use self::error::{Error, ErrorKind, Result};
use crate::service::ServiceState;

/// Build the complete route tree.
///
/// # Route structure
///
/// ```text
/// /health                                       (unversioned)
/// /api/v1/files[/{id}[/content]]
/// /api/v1/policies[/{id}[/{version}|latest]]
/// /api/v1/contexts[/{id}[/{version}|latest]]
/// /api/v1/detections[/{id}[/cancel]]
/// /api/v1/redactions[/{id}]
/// /api/v1/openapi.json                          (added by OpenAPI middleware)
/// /docs                                         (added by OpenAPI middleware)
/// ```
///
/// `/detections` and `/redactions` are filtered views of one
/// underlying run keyspace: a redaction id equals the
/// detection id it was applied from. Bytes (uploads and
/// redacted outputs) flow through `/files`; redacted files
/// carry [`FileLineage::RedactedFrom`] so the run that
/// produced them is traceable.
///
/// [`FileLineage::RedactedFrom`]: nvisy_core::FileLineage::RedactedFrom
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .merge(infra::health_routes())
        .nest("/api/v1", v1_routes())
        .fallback(api_version_fallback)
}

/// All v1 API routes (nested under `/api/v1` by [`routes`]).
fn v1_routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .merge(files::routes_v1())
        .merge(policies::routes_v1())
        .merge(contexts::routes_v1())
        .merge(detections::routes_v1())
        .merge(redactions::routes_v1())
}

/// Catch-all for unmatched paths. Returns 404 with guidance
/// pointing to the current API version for `/api/*` paths.
async fn api_version_fallback(uri: Uri) -> Result<()> {
    let path = uri.path();
    if path.starts_with("/api/") {
        Err(ErrorKind::NotFound.with_message(format!(
            "Unknown API path: {path}. The current API version is v1 at /api/v1/.",
        )))
    } else {
        Err(ErrorKind::NotFound.with_message(format!("Not found: {path}")))
    }
}
