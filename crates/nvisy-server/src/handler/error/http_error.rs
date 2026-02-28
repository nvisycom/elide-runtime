//! HTTP error handling with builder pattern for dynamic error responses.
//!
//! This module provides comprehensive HTTP error handling with a builder pattern
//! that allows for dynamic error messages and resource-specific context.

use std::borrow::Cow;
use std::fmt;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::handler::response::ErrorResponse;

/// The error type for HTTP handlers in the server.
///
/// This error type provides a comprehensive way to handle HTTP errors with proper
/// status codes, messages, and optional context information.
#[derive(Clone)]
#[must_use = "errors do nothing unless serialized"]
pub struct Error<'a> {
    kind: ErrorKind,
    resource: Option<Cow<'a, str>>,
    context: Option<Cow<'a, str>>,
    message: Option<Cow<'a, str>>,
    suggestion: Option<Cow<'a, str>>,
}

impl Error<'static> {
    /// Creates a new [`Error`] with the specified kind.
    #[inline]
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            resource: None,
            context: None,
            message: None,
            suggestion: None,
        }
    }
}

impl<'a> Error<'a> {
    /// Attaches context information to the error.
    ///
    /// Context provides additional information about what went wrong,
    /// which will be included in the error response for debugging.
    #[inline]
    pub fn with_context(self, context: impl Into<Cow<'a, str>>) -> Self {
        Self {
            context: Some(context.into()),
            ..self
        }
    }

    /// Sets a custom user-friendly message for the error.
    #[inline]
    pub fn with_message(self, message: impl Into<Cow<'a, str>>) -> Self {
        Self {
            message: Some(message.into()),
            ..self
        }
    }

    /// Sets the resource that caused the error.
    #[inline]
    pub fn with_resource(self, resource: impl Into<Cow<'a, str>>) -> Self {
        Self {
            resource: Some(resource.into()),
            ..self
        }
    }

    /// Sets a suggestion for how to resolve the error.
    #[inline]
    pub fn with_suggestion(self, suggestion: impl Into<Cow<'a, str>>) -> Self {
        Self {
            suggestion: Some(suggestion.into()),
            ..self
        }
    }

    /// Returns the error kind.
    #[inline]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the context if present.
    #[inline]
    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }

    /// Returns the custom message if present.
    #[inline]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Returns the resource if present.
    #[inline]
    pub fn resource(&self) -> Option<&str> {
        self.resource.as_deref()
    }

    /// Returns the suggestion if present.
    #[inline]
    pub fn suggestion(&self) -> Option<&str> {
        self.suggestion.as_deref()
    }

    /// Converts this error into a static version by cloning all borrowed data.
    pub fn into_owned(self) -> Error<'static> {
        Error {
            kind: self.kind,
            context: self.context.map(|c| Cow::Owned(c.into_owned())),
            message: self.message.map(|m| Cow::Owned(m.into_owned())),
            resource: self.resource.map(|r| Cow::Owned(r.into_owned())),
            suggestion: self.suggestion.map(|s| Cow::Owned(s.into_owned())),
        }
    }
}

impl Default for Error<'static> {
    #[inline]
    fn default() -> Self {
        Self {
            kind: ErrorKind::default(),
            context: None,
            message: None,
            resource: None,
            suggestion: None,
        }
    }
}

impl fmt::Debug for Error<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let response = self.kind.response();

        let mut debug_struct = f.debug_struct("Error");
        debug_struct
            .field("kind", &self.kind)
            .field("name", &response.name)
            .field("status", &response.status)
            .field("message", &response.message)
            .field("resource", &response.resource);

        if let Some(ref context) = self.context {
            debug_struct.field("context", context);
        }

        if let Some(ref message) = self.message {
            debug_struct.field("custom_message", message);
        }

        if let Some(ref resource) = self.resource {
            debug_struct.field("custom_resource", resource);
        }

        if let Some(ref suggestion) = self.suggestion {
            debug_struct.field("suggestion", suggestion);
        }

        debug_struct.finish()
    }
}

impl fmt::Display for Error<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let response = self.kind.response();
        let message = self.message.as_deref().unwrap_or("Unknown error");

        write!(f, "{} ({}): {}", response.name, response.status, message)?;

        if let Some(ref context) = self.context {
            write!(f, " - {}", context)?;
        }

        if let Some(ref resource) = self.resource {
            write!(f, " [resource: {}]", resource)?;
        }

        if let Some(ref suggestion) = self.suggestion {
            write!(f, " | suggestion: {}", suggestion)?;
        }

        Ok(())
    }
}

impl std::error::Error for Error<'_> {}

impl IntoResponse for Error<'_> {
    fn into_response(self) -> Response {
        let mut response = self.kind.response();

        if let Some(message) = self.message {
            response = response.with_message(message);
        }

        if let Some(resource) = self.resource {
            response = response.with_resource(resource);
        }

        if let Some(context) = self.context {
            response = response.with_context(context);
        }

        if let Some(suggestion) = self.suggestion {
            response = response.with_suggestion(suggestion);
        }

        response.into_response()
    }
}

impl From<ErrorKind> for Error<'static> {
    #[inline]
    fn from(kind: ErrorKind) -> Self {
        Self::new(kind)
    }
}

impl From<nvisy_core::Error> for Error<'static> {
    fn from(err: nvisy_core::Error) -> Self {
        let kind = match err.kind {
            nvisy_core::ErrorKind::Validation
            | nvisy_core::ErrorKind::Serialization => ErrorKind::BadRequest,
            nvisy_core::ErrorKind::Policy => ErrorKind::Forbidden,
            nvisy_core::ErrorKind::NotFound => ErrorKind::NotFound,
            nvisy_core::ErrorKind::Connection
            | nvisy_core::ErrorKind::Timeout
            | nvisy_core::ErrorKind::Cancellation
            | nvisy_core::ErrorKind::Runtime
            | nvisy_core::ErrorKind::Internal => ErrorKind::InternalServerError,
        };

        let mut error = Self::new(kind).with_message(err.message);
        if let Some(component) = err.source_component {
            error = error.with_context(component);
        }
        error
    }
}

impl<'a> aide::OperationOutput for Error<'a> {
    type Inner = ErrorResponse<'static>;

    fn operation_response(
        ctx: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) -> Option<aide::openapi::Response> {
        axum::Json::<ErrorResponse<'static>>::operation_response(ctx, operation)
    }

    fn inferred_responses(
        _ctx: &mut aide::generate::GenContext,
        _operation: &mut aide::openapi::Operation,
    ) -> Vec<(Option<aide::openapi::StatusCode>, aide::openapi::Response)> {
        Vec::new()
    }
}

/// A specialized [`Result`] type for HTTP operations.
///
/// This is the standard result type used throughout the nvisy server
/// for operations that can fail with an HTTP error.
///
/// [`Result`]: std::result::Result
pub type Result<T, E = Error<'static>> = std::result::Result<T, E>;

/// Comprehensive enumeration of all possible HTTP error kinds.
///
/// Each variant corresponds to a specific HTTP status code and error scenario.
/// The variants are organized by HTTP status code family.
#[must_use = "error kinds do nothing unless used to create errors"]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    // 4xx Client Errors
    /// 400 Bad Request - Missing required path parameter
    MissingPathParam,
    /// 400 Bad Request - Invalid request data
    BadRequest,
    /// 401 Unauthorized - Missing authentication token
    MissingAuthToken,
    /// 401 Unauthorized - Malformed authentication token
    MalformedAuthToken,
    /// 401 Unauthorized - Invalid credentials
    Unauthorized,
    /// 403 Forbidden - Access denied
    Forbidden,
    /// 404 Not Found - Resource not found
    NotFound,
    /// 409 Conflict - Conflicting resource state
    Conflict,
    /// 429 Too Many Requests - Rate limit exceeded
    TooManyRequests,

    // 5xx Server Errors
    /// 500 Internal Server Error - Unexpected server error
    #[default]
    InternalServerError,
    /// 501 Not Implemented - Feature not yet implemented
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

    /// Returns the base [`ErrorResponse`] for this error kind.
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
        write!(f, "{}", self.response().name.as_ref())
    }
}

impl IntoResponse for ErrorKind {
    #[inline]
    fn into_response(self) -> Response {
        self.response().into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_http_error() {
        let error = Error::default();
        assert_eq!(error.kind(), ErrorKind::InternalServerError);
        let _ = error.into_response();
    }

    #[test]
    fn error_from_kind() {
        let error = Error::new(ErrorKind::NotFound);
        assert_eq!(error.kind(), ErrorKind::NotFound);
        let _ = error.into_response();
    }

    #[test]
    fn error_with_context() {
        let error = ErrorKind::BadRequest.with_context("Invalid format");
        assert_eq!(error.context(), Some("Invalid format"));
        let _ = error.into_response();
    }

    #[test]
    fn error_with_message() {
        let error = ErrorKind::NotFound.with_message("Custom not found message");
        assert_eq!(error.message(), Some("Custom not found message"));
        let _ = error.into_response();
    }

    #[test]
    fn error_with_resource() {
        let error = ErrorKind::Forbidden.with_resource("document");
        assert_eq!(error.resource(), Some("document"));
        let _ = error.into_response();
    }

    #[test]
    fn error_builder_chaining() {
        let error = ErrorKind::NotFound
            .with_message("Document not found")
            .with_resource("document")
            .with_context("ID: 123")
            .with_suggestion("Check if the document ID is correct");

        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert_eq!(error.message(), Some("Document not found"));
        assert_eq!(error.resource(), Some("document"));
        assert_eq!(error.context(), Some("ID: 123"));
        assert_eq!(
            error.suggestion(),
            Some("Check if the document ID is correct")
        );
    }

    #[test]
    fn std_fmt_display() {
        let error = ErrorKind::NotFound
            .with_message("Resource not found")
            .with_resource("document")
            .with_context("ID: 123");

        let display = format!("{}", error);
        assert!(display.contains("not_found"));
        assert!(display.contains("404"));
        assert!(display.contains("Resource not found"));
        assert!(display.contains("ID: 123"));
        assert!(display.contains("document"));
    }

    #[test]
    fn std_fmt_debug() {
        let error = ErrorKind::Forbidden
            .with_message("Access denied")
            .with_resource("document")
            .with_context("User lacks permissions");

        let debug = format!("{:?}", error);
        assert!(debug.contains("Forbidden"));
        assert!(debug.contains("Access denied"));
        assert!(debug.contains("document"));
    }

    #[test]
    fn std_error_trait() {
        let error = Error::new(ErrorKind::BadRequest);
        let _: &dyn std::error::Error = &error;
    }

    #[test]
    fn error_into_static() {
        let error = ErrorKind::NotFound
            .with_message("Test message".to_string())
            .with_resource("test_resource".to_string())
            .with_context("Test context".to_string())
            .with_suggestion("Test suggestion".to_string());

        let static_error = error.into_owned();
        assert_eq!(static_error.message(), Some("Test message"));
        assert_eq!(static_error.resource(), Some("test_resource"));
        assert_eq!(static_error.context(), Some("Test context"));
        assert_eq!(static_error.suggestion(), Some("Test suggestion"));
    }

    #[test]
    fn all_error_kinds_have_responses() {
        let kinds = vec![
            ErrorKind::BadRequest,
            ErrorKind::Conflict,
            ErrorKind::Forbidden,
            ErrorKind::InternalServerError,
            ErrorKind::MalformedAuthToken,
            ErrorKind::MissingAuthToken,
            ErrorKind::MissingPathParam,
            ErrorKind::NotFound,
            ErrorKind::NotImplemented,
            ErrorKind::Unauthorized,
        ];

        for kind in kinds {
            let response = kind.response();
            assert!(!response.name.is_empty());
            assert!(response.status.as_u16() >= 400);
            let _ = kind.into_response();
        }
    }

    #[test]
    fn from_nvisy_core_validation() {
        let core_err = nvisy_core::Error::new(
            nvisy_core::ErrorKind::Validation,
            "field is required",
        );
        let err = Error::from(core_err);
        assert_eq!(err.kind(), ErrorKind::BadRequest);
        assert_eq!(err.message(), Some("field is required"));
    }

    #[test]
    fn from_nvisy_core_not_found() {
        let core_err = nvisy_core::Error::new(nvisy_core::ErrorKind::NotFound, "missing")
            .with_component("registry");
        let err = Error::from(core_err);
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(err.message(), Some("missing"));
        assert_eq!(err.context(), Some("registry"));
    }

    #[test]
    fn from_nvisy_core_internal() {
        let core_err =
            nvisy_core::Error::new(nvisy_core::ErrorKind::Runtime, "unexpected");
        let err = Error::from(core_err);
        assert_eq!(err.kind(), ErrorKind::InternalServerError);
    }
}
