//! HTTP handler functions and route wiring.
//!
//! Each submodule corresponds to an API resource and exposes a `routes()`
//! function that returns its [`ApiRouter`](aide::axum::ApiRouter) fragment.
//! The top-level [`routes()`] function merges all fragments into a single
//! router.
//!
//! Request and response types live in the [`request`] and [`response`]
//! submodules. [`Error`], [`ErrorKind`], and [`Result`] are re-exported for
//! use by middleware and extractors.

pub mod error;
pub mod utility;

mod check;
mod contexts;
mod files;
mod process;

mod request;
mod response;

use aide::axum::ApiRouter;
pub use error::{Error, ErrorKind, Result};

use crate::service::ServiceState;

/// Build the handler route tree.
pub fn routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .merge(check::routes())
        .merge(contexts::routes())
        .merge(files::routes())
        .merge(process::routes())
}
