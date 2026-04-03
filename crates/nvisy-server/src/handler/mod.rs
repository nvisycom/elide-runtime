//! HTTP handler functions and route wiring.
//!
//! Each submodule corresponds to an API resource and exposes a
//! `routes_v1()` function that returns relative-path `ApiRouter`
//! fragments. These are nested under `/api/v1` by [`routes`].
//!
//! The top-level [`routes()`] function assembles all versions plus
//! unversioned routes (health, docs) into a single router.
//!
//! Request and response types live in the `request` and `response`
//! submodules. [`Error`], [`ErrorKind`], and [`Result`] are re-exported for
//! use by middleware and extractors.

pub mod error;
pub mod utility;

mod contexts;
mod files;
mod infra;
mod policies;
mod runs;

mod request;
mod response;

use aide::axum::ApiRouter;

pub use self::error::{Error, ErrorKind, Result};
use crate::service::ServiceState;

/// Build the complete route tree with versioned API and unversioned infra routes.
///
/// # Route structure
///
/// ```text
/// /health                      (unversioned)
/// /api/v1/analytics
/// /api/v1/contexts[/{id}]
/// /api/v1/files[/{id}]
/// /api/v1/policies[/{id}]
/// /api/v1/runs[/{id}[/cancel]]
/// /api/v1/openapi.json         (added by OpenAPI middleware)
/// /docs                        (added by OpenAPI middleware)
/// ```
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .merge(infra::health_routes())
        .nest("/api/v1", v1_routes())
        .fallback(api_version_fallback)
}

/// All v1 API routes (nested under `/api/v1` by [`routes`]).
///
/// When a new API version is introduced, a parallel `v2_routes()`
/// function can compose a different set of handlers under `/api/v2`.
fn v1_routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .merge(infra::routes_v1())
        .merge(contexts::routes_v1())
        .merge(files::routes_v1())
        .merge(policies::routes_v1())
        .merge(runs::routes_v1())
}

/// Catch-all for unmatched paths.
///
/// Returns 404 with guidance pointing to the current API version
/// for `/api/*` paths.
async fn api_version_fallback(uri: axum::http::Uri) -> Result<()> {
    let path = uri.path();
    if path.starts_with("/api/") {
        Err(ErrorKind::NotFound.with_message(format!(
            "Unknown API path: {path}. The current API version is v1 at /api/v1/.",
        )))
    } else {
        Err(ErrorKind::NotFound.with_message(format!("Not found: {path}")))
    }
}
