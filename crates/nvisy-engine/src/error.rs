//! Unified error types for the nvisy platform.
//!
//! All crates in the nvisy workspace use [`Error`] as their primary error
//! type and [`ErrorKind`] to classify failures.
//!
//! Construction goes through [`Error::new`] or one of the per-kind
//! shorthand fns ([`Error::validation`], [`Error::not_found`], …).
//! There is one helper per [`ErrorKind`] variant. Each shorthand
//! seeds the `retryable` flag with the sensible default for that
//! kind (timeouts and transient connection failures default to
//! retryable; everything else defaults to non-retryable).
//!
//! After construction, attach context via the builder methods:
//! [`with_source`], [`with_component`], [`with_retryable`]. Read
//! state back via the accessors ([`kind`], [`message`], [`component`],
//! [`is_retryable`]); the underlying cause is reachable through the
//! standard [`Error::source`] method.
//!
//! [`Error::source`]: std::error::Error::source
//!
//! [`with_source`]: Error::with_source
//! [`with_component`]: Error::with_component
//! [`with_retryable`]: Error::with_retryable
//! [`kind`]: Error::kind
//! [`message`]: Error::message
//! [`component`]: Error::component
//! [`is_retryable`]: Error::is_retryable

use std::borrow::Cow;
use std::{error, io, result};

use strum::Display;

/// Trait-object alias for the [`Error`] cause chain.
///
/// Wraps any `std::error::Error` that's safe to send across
/// threads: the usual bound for error sources in async code.
pub type ErrorSource = Box<dyn error::Error + Send + Sync>;

/// Classification of error kinds.
///
/// Used to tag every [`Error`] so callers can programmatically decide
/// how to handle a failure (e.g. retry on `Timeout`, surface to user
/// on `Validation`). Grouped by failure domain:
///
/// - **Domain failures** (`Validation`, `Policy`, `NotFound`,
///   `Conflict`). The operation was well-formed but rejected by domain
///   logic.
/// - **Transport failures** (`Connection`, `Timeout`, `Cancellation`).
///   The operation never completed because the channel failed; often
///   retryable.
/// - **Infrastructure failures** (`Internal`, `Runtime`, `Serialization`).
///   Something inside the process or its immediate dependencies broke;
///   not the caller's fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
#[strum(serialize_all = "snake_case")]
pub enum ErrorKind {
    /// Input or configuration failed validation checks.
    Validation,
    /// A policy rule was violated.
    Policy,
    /// The requested resource was not found.
    NotFound,
    /// The operation conflicts with the resource's current state.
    ///
    /// E.g. "already in terminal state", "cannot delete while
    /// running". Maps to HTTP 409. Non-retryable: the caller has
    /// to inspect the resource and pick a different operation.
    Conflict,
    /// Could not connect to an external service.
    Connection,
    /// An operation exceeded its time limit.
    Timeout,
    /// The operation was explicitly cancelled.
    Cancellation,
    /// An internal infrastructure error (filesystem, I/O, fjall).
    Internal,
    /// An internal runtime error inside an engine operation.
    Runtime,
    /// A serialization or encoding error.
    Serialization,
}

/// Unified error type for the nvisy platform.
///
/// Carries a [`kind`], a human-readable [`message`], an optional
/// [`component`] tag identifying the producer (e.g. `"detection"`,
/// `"registry"`), an [`is_retryable`] flag, and an optional wrapped
/// source error reachable through [`Error::source`].
///
/// Fields are private; construct with [`Error::new`] or a per-kind
/// shorthand, then layer context via [`with_source`], [`with_component`],
/// [`with_retryable`].
///
/// [`kind`]: Self::kind
/// [`message`]: Self::message
/// [`component`]: Self::component
/// [`is_retryable`]: Self::is_retryable
/// [`with_source`]: Self::with_source
/// [`with_component`]: Self::with_component
/// [`with_retryable`]: Self::with_retryable
/// [`Error::source`]: std::error::Error::source
#[derive(Debug, thiserror::Error)]
#[error("{kind}: {message}")]
pub struct Error {
    kind: ErrorKind,
    message: String,
    component: Option<Cow<'static, str>>,
    retryable: bool,
    #[source]
    source: Option<ErrorSource>,
}

impl Error {
    /// Construct an error with the given kind and message.
    ///
    /// No component, no source, `retryable = false`. Prefer the
    /// per-kind shorthand fns ([`Self::validation`],
    /// [`Self::timeout`], …) when one matches; they set the right
    /// `retryable` default for that kind.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            component: None,
            retryable: false,
            source: None,
        }
    }

    /// Attach an underlying cause to this error.
    ///
    /// Reachable downstream via [`Error::source`].
    ///
    /// [`Error::source`]: std::error::Error::source
    pub fn with_source(mut self, source: impl error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// Tag this error with the name of the producer component.
    ///
    /// E.g. `"detection"`, `"registry"`, `"ocr-bento"`. Accepts
    /// `&'static str` (zero-alloc) or `String` (when the name is
    /// computed at runtime).
    pub fn with_component(mut self, component: impl Into<Cow<'static, str>>) -> Self {
        self.component = Some(component.into());
        self
    }

    /// Override the retryable flag.
    ///
    /// Per-kind shorthand fns set sensible defaults; use this
    /// only when the call site has information the kind alone
    /// can't express.
    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    /// The error's kind classification.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The human-readable message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The producer component tag, if attached.
    pub fn component(&self) -> Option<&str> {
        self.component.as_deref()
    }

    /// Whether the operation that failed can be safely retried.
    pub fn is_retryable(&self) -> bool {
        self.retryable
    }

    /// Validation failure. Non-retryable.
    ///
    /// Caller's input or configuration was rejected by domain
    /// logic.
    pub fn validation(message: impl Into<String>, component: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ErrorKind::Validation, message).with_component(component)
    }

    /// Policy violation. Non-retryable.
    ///
    /// Detected data conflicts with an active policy rule.
    pub fn policy(message: impl Into<String>, component: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ErrorKind::Policy, message).with_component(component)
    }

    /// Resource not found. Non-retryable.
    pub fn not_found(message: impl Into<String>, component: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ErrorKind::NotFound, message).with_component(component)
    }

    /// Resource-state conflict. Non-retryable.
    ///
    /// E.g. an operation that requires a different status. Maps
    /// to HTTP 409.
    pub fn conflict(message: impl Into<String>, component: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ErrorKind::Conflict, message).with_component(component)
    }

    /// Connection failure to an external service.
    ///
    /// `retryable` is caller-determined: transient network
    /// glitches are retryable, permanent auth failures are not.
    pub fn connection(
        message: impl Into<String>,
        component: impl Into<Cow<'static, str>>,
        retryable: bool,
    ) -> Self {
        Self::new(ErrorKind::Connection, message)
            .with_component(component)
            .with_retryable(retryable)
    }

    /// Timeout. Always retryable.
    pub fn timeout(message: impl Into<String>, component: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ErrorKind::Timeout, message)
            .with_component(component)
            .with_retryable(true)
    }

    /// Explicit cancellation. Non-retryable.
    ///
    /// By definition the caller asked us to stop.
    pub fn cancellation(
        message: impl Into<String>,
        component: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::new(ErrorKind::Cancellation, message).with_component(component)
    }

    /// Internal infrastructure failure (filesystem, I/O, database).
    ///
    /// Non-retryable by default: most internal failures need
    /// investigation, not retry.
    pub fn internal(message: impl Into<String>, component: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ErrorKind::Internal, message).with_component(component)
    }

    /// Runtime failure inside an engine operation.
    ///
    /// `retryable` is caller-determined: an LLM rate-limit is
    /// retryable, a compile-time pattern error is not.
    pub fn runtime(
        message: impl Into<String>,
        component: impl Into<Cow<'static, str>>,
        retryable: bool,
    ) -> Self {
        Self::new(ErrorKind::Runtime, message)
            .with_component(component)
            .with_retryable(retryable)
    }

    /// Serialization / encoding failure. Non-retryable.
    pub fn serialization(
        message: impl Into<String>,
        component: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::new(ErrorKind::Serialization, message).with_component(component)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::internal(err.to_string(), "io").with_source(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::serialization(err.to_string(), "serde_json").with_source(err)
    }
}

impl From<derive_builder::UninitializedFieldError> for Error {
    fn from(err: derive_builder::UninitializedFieldError) -> Self {
        Self::validation(
            format!("missing required field `{}`", err.field_name()),
            "derive_builder",
        )
    }
}

impl From<elide_core::Error> for Error {
    /// Map elide's per-operation error into the runtime's shared vocabulary.
    ///
    /// The elide `ErrorKind` is preserved semantically via a
    /// mapping onto the nearest [`ErrorKind`] variant; the
    /// original elide error travels along as the source cause.
    ///
    /// Called at every `nvisy-engine` seam where an `elide::Error`
    /// crosses into engine-land — pattern compile, recognizer
    /// build, anonymizer attach, orchestrator analyze/anonymize.
    fn from(err: elide_core::Error) -> Self {
        let kind = match err.kind() {
            elide_core::ErrorKind::OutOfRange | elide_core::ErrorKind::Validation => {
                ErrorKind::Validation
            }
            elide_core::ErrorKind::Transport => ErrorKind::Connection,
            _ => ErrorKind::Runtime,
        };
        Self::new(kind, err.to_string()).with_source(err)
    }
}

/// Convenience type alias for results using the Nvisy error type.
pub type Result<T, E = Error> = result::Result<T, E>;
