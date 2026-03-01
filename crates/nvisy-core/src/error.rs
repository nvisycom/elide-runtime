//! Unified error types for the nvisy platform.
//!
//! All crates in the nvisy workspace use [`Error`] as their primary error
//! type and [`ErrorKind`] to classify failures.

use derive_more::Display;

/// Classification of error kinds.
///
/// Used to tag every [`Error`] so callers can programmatically decide
/// how to handle a failure (e.g. retry on `Timeout`, surface to user
/// on `Validation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum ErrorKind {
    /// Input or configuration failed validation checks.
    Validation,
    /// Could not connect to an external service.
    Connection,
    /// An operation exceeded its time limit.
    Timeout,
    /// The operation was explicitly cancelled.
    Cancellation,
    /// A policy rule was violated.
    Policy,
    /// An internal runtime error occurred.
    Runtime,
    /// An internal infrastructure error (filesystem, I/O).
    Internal,
    /// A serialization or encoding error.
    Serialization,
    /// The requested resource was not found.
    NotFound,
}

/// Unified error type for the nvisy platform.
///
/// Carries a [`kind`](ErrorKind), a human-readable message, an optional
/// source component name, a retryable flag, and an optional wrapped cause.
#[derive(Debug, thiserror::Error)]
#[error("{kind}: {message}")]
pub struct Error {
    /// Classification of the error.
    pub kind: ErrorKind,
    /// Human-readable description of what went wrong.
    pub message: String,
    /// Name of the component that produced this error (e.g. `"s3-read"`, `"detect-regex"`).
    pub source_component: Option<String>,
    /// Whether the operation that failed can be safely retried.
    pub retryable: bool,
    /// The underlying cause, if any.
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl Error {
    /// Create a new error with the given kind and message.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source_component: None,
            retryable: false,
            source: None,
        }
    }

    /// Attach an underlying cause to this error.
    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// Tag this error with the name of the component that produced it.
    pub fn with_component(mut self, component: impl Into<String>) -> Self {
        self.source_component = Some(component.into());
        self
    }

    /// Mark whether this error is safe to retry.
    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    /// Shorthand for a validation error with a source component.
    pub fn validation(message: impl Into<String>, source: impl Into<String>) -> Self {
        Self::new(ErrorKind::Validation, message).with_component(source)
    }

    /// Shorthand for a connection error with a source component and retryable flag.
    pub fn connection(
        message: impl Into<String>,
        source: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self::new(ErrorKind::Connection, message)
            .with_component(source)
            .with_retryable(retryable)
    }

    /// Shorthand for a timeout error (always retryable).
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Timeout, message).with_retryable(true)
    }

    /// Shorthand for a cancellation error.
    pub fn cancellation(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Cancellation, message)
    }

    /// Shorthand for a policy violation error.
    pub fn policy(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Policy, message)
    }

    /// Shorthand for a runtime error with a source component and retryable flag.
    pub fn runtime(
        message: impl Into<String>,
        source: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self::new(ErrorKind::Runtime, message)
            .with_component(source)
            .with_retryable(retryable)
    }

    /// Whether this error is retryable.
    pub fn is_retryable(&self) -> bool {
        self.retryable
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::new(ErrorKind::Internal, err.to_string())
            .with_source(err)
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        // anyhow::Error doesn't implement std::error::Error, so we capture the
        // full chain as text instead of storing it as a boxed source.
        Self::new(ErrorKind::Runtime, format!("{err:#}"))
    }
}

/// Convenience type alias for results using the Nvisy error type.
pub type Result<T> = std::result::Result<T, Error>;
