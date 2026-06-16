//! HTTP error type with builder pattern for dynamic error responses.

use std::borrow::Cow;
use std::{error, fmt, result};

use aide::generate::GenContext;
use aide::openapi::{Operation, Response as OpenApiResponse, StatusCode};
use axum::response::{IntoResponse, Response};

use super::http_kind::ErrorKind;
use crate::handler::response::ErrorResponse;

/// The error type for HTTP handlers in the server.
///
/// Carries an [`ErrorKind`], optional message, resource, context, and
/// suggestion. Converts into an axum [`Response`] via [`IntoResponse`].
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
        let mut debug_struct = f.debug_struct("Error");
        debug_struct
            .field("kind", &self.kind)
            .field("status", &self.kind.status_code());

        if let Some(ref message) = self.message {
            debug_struct.field("message", message);
        }

        if let Some(ref resource) = self.resource {
            debug_struct.field("resource", resource);
        }

        if let Some(ref context) = self.context {
            debug_struct.field("context", context);
        }

        if let Some(ref suggestion) = self.suggestion {
            debug_struct.field("suggestion", suggestion);
        }

        debug_struct.finish()
    }
}

impl fmt::Display for Error<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.kind.response().name;
        let status = self.kind.status_code();
        let message = self.message.as_deref().unwrap_or("Unknown error");

        write!(f, "{name} ({status}): {message}")?;

        if let Some(ref context) = self.context {
            write!(f, " - {context}")?;
        }

        if let Some(ref resource) = self.resource {
            write!(f, " [resource: {resource}]")?;
        }

        if let Some(ref suggestion) = self.suggestion {
            write!(f, " | suggestion: {suggestion}")?;
        }

        Ok(())
    }
}

impl error::Error for Error<'_> {}

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

impl<'a> aide::OperationOutput for Error<'a> {
    type Inner = ErrorResponse<'static>;

    fn operation_response(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Option<OpenApiResponse> {
        axum::Json::<ErrorResponse<'static>>::operation_response(ctx, operation)
    }

    fn inferred_responses(
        _ctx: &mut GenContext,
        _operation: &mut Operation,
    ) -> Vec<(Option<StatusCode>, OpenApiResponse)> {
        Vec::new()
    }
}

/// A specialized [`Result`] type for HTTP operations.
///
/// [`Result`]: std::result::Result
pub type Result<T, E = Error<'static>> = result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn std_fmt_display() {
        let error = ErrorKind::NotFound
            .with_message("Resource not found")
            .with_resource("document")
            .with_context("ID: 123");

        let display = format!("{error}");
        assert!(display.contains("NOT_FOUND"));
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

        let debug = format!("{error:?}");
        assert!(debug.contains("Forbidden"));
        assert!(debug.contains("Access denied"));
        assert!(debug.contains("document"));
    }
}
