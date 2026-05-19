//! HTTP error kind enumeration and status code mapping.

use std::borrow::Cow;
use std::fmt;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::http_error::Error;
use crate::handler::response::ErrorResponse;

/// Enumeration of all possible HTTP error kinds.
///
/// Each variant corresponds to a specific HTTP status code and error scenario.
#[must_use = "error kinds do nothing unless used to create errors"]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// 400 Bad Request: missing required path parameter.
    MissingPathParam,
    /// 400 Bad Request: invalid request data.
    BadRequest,
    /// 401 Unauthorized: missing authentication token.
    MissingAuthToken,
    /// 401 Unauthorized: malformed authentication token.
    MalformedAuthToken,
    /// 401 Unauthorized: invalid credentials.
    Unauthorized,
    /// 403 Forbidden: access denied.
    Forbidden,
    /// 404 Not Found: resource not found.
    NotFound,
    /// 409 Conflict: conflicting resource state.
    Conflict,
    /// 429 Too Many Requests: rate limit exceeded.
    TooManyRequests,
    /// 500 Internal Server Error: unexpected server error.
    #[default]
    InternalServerError,
    /// 501 Not Implemented: feature not yet implemented.
    NotImplemented,
}

impl ErrorKind {
    /// Converts this error kind into a full [`Error`].
    #[inline]
    pub fn into_error(self) -> Error<'static> {
        Error::new(self)
    }

    /// Creates an [`Error`] with the specified context.
    #[inline]
    pub fn with_context<'a>(self, context: impl Into<Cow<'a, str>>) -> Error<'a> {
        Error::new(self).with_context(context)
    }

    /// Creates an [`Error`] with the specified message.
    #[inline]
    pub fn with_message<'a>(self, message: impl Into<Cow<'a, str>>) -> Error<'a> {
        Error::new(self).with_message(message)
    }

    /// Creates an [`Error`] with the specified resource.
    #[inline]
    pub fn with_resource<'a>(self, resource: impl Into<Cow<'a, str>>) -> Error<'a> {
        Error::new(self).with_resource(resource)
    }

    /// Creates an [`Error`] with the specified suggestion.
    #[inline]
    pub fn with_suggestion<'a>(self, suggestion: impl Into<Cow<'a, str>>) -> Error<'a> {
        Error::new(self).with_suggestion(suggestion)
    }

    /// Returns the HTTP status code for this error kind.
    #[inline]
    pub fn status_code(self) -> StatusCode {
        self.response().status
    }

    /// Returns the base `ErrorResponse` for this error kind.
    #[inline]
    pub fn response(self) -> ErrorResponse<'static> {
        match self {
            Self::MissingPathParam => ErrorResponse::MISSING_PATH_PARAM,
            Self::BadRequest => ErrorResponse::BAD_REQUEST,
            Self::MissingAuthToken => ErrorResponse::MISSING_AUTH_TOKEN,
            Self::MalformedAuthToken => ErrorResponse::MALFORMED_AUTH_TOKEN,
            Self::Unauthorized => ErrorResponse::UNAUTHORIZED,
            Self::Forbidden => ErrorResponse::FORBIDDEN,
            Self::NotFound => ErrorResponse::NOT_FOUND,
            Self::Conflict => ErrorResponse::CONFLICT,
            Self::TooManyRequests => ErrorResponse::TOO_MANY_REQUESTS,
            Self::InternalServerError => ErrorResponse::INTERNAL_SERVER_ERROR,
            Self::NotImplemented => ErrorResponse::NOT_IMPLEMENTED,
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &*self.response().name)
    }
}

impl IntoResponse for ErrorKind {
    #[inline]
    fn into_response(self) -> Response {
        self.response().into_response()
    }
}
