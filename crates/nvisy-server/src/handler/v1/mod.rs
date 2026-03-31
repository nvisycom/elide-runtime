//! API version 1 route composition.
//!
//! Nests all handler modules under the `/api/v1` prefix. When a new
//! API version is introduced, a parallel `v2.rs` module can compose a
//! different set of handlers (or share them) under `/api/v2`.

use aide::axum::ApiRouter;

use super::{contexts, files, infra, runs};
use crate::service::ServiceState;

/// All v1 API routes (relative to `/api/v1`).
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .merge(infra::analytics_routes())
        .merge(contexts::routes())
        .merge(files::routes())
        .merge(runs::routes())
}
