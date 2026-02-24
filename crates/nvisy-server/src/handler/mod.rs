//! HTTP handler functions and route wiring.
//!
//! Each submodule corresponds to an API resource. Request and response types
//! live in the [`request`] and [`response`] submodules respectively.

mod check;
mod execute;
mod ingest;
mod redact;

pub mod request;
pub mod response;

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
