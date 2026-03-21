//! Serializable error response body for API endpoints.
//!
//! [`ErrorResponse`] is the JSON body returned by every error path.
//! It carries a stable machine-readable `name`, the HTTP `status` code,
//! and optional human-readable fields (`message`, `resource`, `context`,
//! `suggestion`).

use std::borrow::Cow;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use schemars::JsonSchema;
use serde::Serialize;

/// JSON error body returned by all API endpoints.
///
/// Designed for both human readability and machine parsing. The `name`
/// field is a stable, machine-readable identifier; `status` is the
/// HTTP status code; `message` is a human-readable description.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse<'a> {
    /// Machine-readable error name (e.g. `"NOT_FOUND"`, `"BAD_REQUEST"`).
    pub name: Cow<'a, str>,
    /// HTTP status code (conveyed via the response status line, not the body).
    #[serde(skip)]
    #[schemars(skip)]
    pub status: StatusCode,
    /// Human-readable error message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Cow<'a, str>>,
    /// The resource the error relates to (e.g. a path or ID).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<Cow<'a, str>>,
    /// Contextual information (e.g. component name, operation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Cow<'a, str>>,
    /// A suggested action the client could take.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<Cow<'a, str>>,
}

impl ErrorResponse<'static> {
    pub const BAD_REQUEST: Self = Self {
        name: Cow::Borrowed("bad_request"),
        status: StatusCode::BAD_REQUEST,
        message: Some(Cow::Borrowed("The request is invalid or malformed")),
        resource: None,
        context: None,
        suggestion: Some(Cow::Borrowed("Check the request body and query parameters")),
    };
    pub const CONFLICT: Self = Self {
        name: Cow::Borrowed("conflict"),
        status: StatusCode::CONFLICT,
        message: Some(Cow::Borrowed(
            "The request conflicts with the current resource state",
        )),
        resource: None,
        context: None,
        suggestion: Some(Cow::Borrowed(
            "The resource may have been modified; retry the operation",
        )),
    };
    pub const FORBIDDEN: Self = Self {
        name: Cow::Borrowed("forbidden"),
        status: StatusCode::FORBIDDEN,
        message: Some(Cow::Borrowed("Access to this resource is denied")),
        resource: None,
        context: None,
        suggestion: Some(Cow::Borrowed("Check your permissions for this resource")),
    };
    pub const INTERNAL_SERVER_ERROR: Self = Self {
        name: Cow::Borrowed("internal_server_error"),
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: Some(Cow::Borrowed("An unexpected error occurred")),
        resource: None,
        context: None,
        suggestion: Some(Cow::Borrowed(
            "Retry the request; contact support if the issue persists",
        )),
    };
    pub const MALFORMED_AUTH_TOKEN: Self = Self {
        name: Cow::Borrowed("malformed_auth_token"),
        status: StatusCode::UNAUTHORIZED,
        message: Some(Cow::Borrowed("The authentication token is malformed")),
        resource: None,
        context: None,
        suggestion: Some(Cow::Borrowed("Ensure the token is a valid UUID")),
    };
    pub const MISSING_AUTH_TOKEN: Self = Self {
        name: Cow::Borrowed("missing_auth_token"),
        status: StatusCode::UNAUTHORIZED,
        message: Some(Cow::Borrowed("No authentication token was provided")),
        resource: None,
        context: None,
        suggestion: Some(Cow::Borrowed(
            "Include the X-Actor-Id header in the request",
        )),
    };
    pub const MISSING_PATH_PARAM: Self = Self {
        name: Cow::Borrowed("missing_path_param"),
        status: StatusCode::BAD_REQUEST,
        message: Some(Cow::Borrowed(
            "A required path parameter is missing or invalid",
        )),
        resource: None,
        context: None,
        suggestion: Some(Cow::Borrowed(
            "Include the required path parameter in the URL",
        )),
    };
    pub const NOT_FOUND: Self = Self {
        name: Cow::Borrowed("not_found"),
        status: StatusCode::NOT_FOUND,
        message: Some(Cow::Borrowed("The requested resource was not found")),
        resource: None,
        context: None,
        suggestion: Some(Cow::Borrowed("Verify the resource identifier exists")),
    };
    pub const NOT_IMPLEMENTED: Self = Self {
        name: Cow::Borrowed("not_implemented"),
        status: StatusCode::NOT_IMPLEMENTED,
        message: Some(Cow::Borrowed("This operation is not yet implemented")),
        resource: None,
        context: None,
        suggestion: Some(Cow::Borrowed("This feature is not yet available")),
    };
    pub const TOO_MANY_REQUESTS: Self = Self {
        name: Cow::Borrowed("too_many_requests"),
        status: StatusCode::TOO_MANY_REQUESTS,
        message: Some(Cow::Borrowed("Rate limit exceeded")),
        resource: None,
        context: None,
        suggestion: Some(Cow::Borrowed("Wait before retrying")),
    };
    pub const UNAUTHORIZED: Self = Self {
        name: Cow::Borrowed("unauthorized"),
        status: StatusCode::UNAUTHORIZED,
        message: Some(Cow::Borrowed("Authentication is required")),
        resource: None,
        context: None,
        suggestion: Some(Cow::Borrowed("Provide valid credentials in the request")),
    };
}

impl<'a> ErrorResponse<'a> {
    /// Sets a human-readable error message.
    pub fn with_message(mut self, message: impl Into<Cow<'a, str>>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Sets the resource the error relates to.
    pub fn with_resource(mut self, resource: impl Into<Cow<'a, str>>) -> Self {
        self.resource = Some(resource.into());
        self
    }

    /// Sets contextual information about the error.
    pub fn with_context(mut self, context: impl Into<Cow<'a, str>>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Sets a suggestion for how to resolve the error.
    pub fn with_suggestion(mut self, suggestion: impl Into<Cow<'a, str>>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl IntoResponse for ErrorResponse<'_> {
    fn into_response(self) -> Response {
        (self.status, axum::Json(self)).into_response()
    }
}
