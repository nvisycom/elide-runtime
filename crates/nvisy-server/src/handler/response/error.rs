//! Unified error handling.
//!
//! Maps [`ErrorKind`] to HTTP status codes and produces a JSON error body
//! compatible with the OpenAPI specification. Every handler returns
//! `Result<Json<T>, ServerError>` so that errors are serialised uniformly.
//!
//! | [`ErrorKind`]                                  | HTTP Status            |
//! |------------------------------------------------|------------------------|
//! | `Validation`, `InvalidInput`, `Serialization`  | 400 Bad Request        |
//! | `Policy`                                       | 403 Forbidden          |
//! | `NotFound`                                     | 404 Not Found          |
//! | `Connection`                                   | 502 Bad Gateway        |
//! | `Timeout`                                      | 504 Gateway Timeout    |
//! | `Cancellation`                                 | 499 Client Closed      |
//! | `Runtime`, `Python`, `InternalError`, `Other`  | 500 Internal Server    |

use aide::OperationOutput;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use nvisy_core::{Error, ErrorKind};
use schemars::JsonSchema;
use serde::Serialize;

/// JSON error body returned by all endpoints.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ApiError {
    pub error: ApiErrorBody,
}

/// Inner error payload.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ApiErrorBody {
    pub kind: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    pub retryable: bool,
}

impl From<Error> for ApiError {
    fn from(err: Error) -> Self {
        Self {
            error: ApiErrorBody {
                kind: err.kind.to_string(),
                message: err.message,
                component: err.source_component,
                retryable: err.retryable,
            },
        }
    }
}

/// Map [`ErrorKind`] to an HTTP status code.
fn status_for(kind: ErrorKind) -> StatusCode {
    match kind {
        ErrorKind::Validation | ErrorKind::InvalidInput | ErrorKind::Serialization => {
            StatusCode::BAD_REQUEST
        }
        ErrorKind::Policy => StatusCode::FORBIDDEN,
        ErrorKind::NotFound => StatusCode::NOT_FOUND,
        ErrorKind::Connection => StatusCode::BAD_GATEWAY,
        ErrorKind::Timeout => StatusCode::GATEWAY_TIMEOUT,
        // 499: non-standard "Client Closed Request"
        ErrorKind::Cancellation => StatusCode::from_u16(499).unwrap_or(StatusCode::BAD_REQUEST),
        ErrorKind::Runtime | ErrorKind::Python | ErrorKind::InternalError | ErrorKind::Other => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Newtype wrapper so we can implement `IntoResponse` for `nvisy_core::Error`.
pub struct ServerError(pub Error);

impl From<Error> for ServerError {
    fn from(err: Error) -> Self {
        Self(err)
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = status_for(self.0.kind);
        tracing::warn!(
            kind = %self.0.kind,
            status = status.as_u16(),
            component = self.0.source_component.as_deref(),
            retryable = self.0.retryable,
            "{}",
            self.0.message,
        );
        let body: ApiError = self.0.into();
        (status, axum::Json(body)).into_response()
    }
}

impl OperationOutput for ServerError {
    type Inner = ApiError;
}
