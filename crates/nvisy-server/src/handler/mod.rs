//! HTTP handler functions and route wiring.
//!
//! Each submodule corresponds to an API resource and exposes a `routes()`
//! function that returns its [`ApiRouter`](aide::axum::ApiRouter) fragment.
//! Request and response types live in the [`request`] and [`response`]
//! submodules respectively.
//!
//! | Module     | Endpoints                                           |
//! |------------|-----------------------------------------------------|
//! | [`check`]  | `GET /health`, `GET /api/v1/analytics`               |
//! | [`execute`]| `POST /api/v1/execute`                               |
//! | [`ingest`] | `POST /api/v1/ingest`, `GET /api/v1/ingest/{id}`    |
//! | [`redact`] | `POST /api/v1/redaction`                             |

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
