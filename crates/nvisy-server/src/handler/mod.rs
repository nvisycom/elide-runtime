//! HTTP handler functions and route wiring.
//!
//! Each submodule corresponds to an API resource and exposes a `routes()`
//! function that returns its [`ApiRouter`](aide::axum::ApiRouter) fragment.
//! The top-level [`routes()`] function merges all fragments into a single
//! router.
//!
//! Request and response types live in the private [`request`] and [`response`]
//! submodules. Only [`ServerError`] is re-exported for use by middleware.

mod check;
mod execute;
mod ingest;
mod redact;

mod request;
mod response;

pub use response::ServerError;

use aide::axum::ApiRouter;

use crate::service::ServiceState;

/// Build the handler route tree.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .merge(check::routes())
        .merge(execute::routes())
        .merge(ingest::routes())
        .merge(redact::routes())
}
