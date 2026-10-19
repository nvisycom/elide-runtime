//! What can go wrong loading a deployment.
//!
//! Deliberately its own type rather than [`elide::Error`]: the
//! failures here are about a *configuration document* — a syntax
//! error at a line and column, an environment variable a file
//! referenced but the process does not have — and none of them
//! describe a document being redacted. A host surfaces these at
//! startup, not per request.

use std::fmt;

/// What kind of configuration failure occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The document is not well-formed in its own syntax, or does
    /// not match the deployment schema.
    Malformed,
    /// The document interpolates `${VAR}`, and `VAR` is not set in
    /// the process environment.
    ///
    /// Always an error, never an empty string: a missing credential
    /// that silently became `""` would be sent to the backend as a
    /// credential, and the failure would surface as an
    /// authentication error far from its cause.
    MissingVariable,
    /// The document is well-formed but describes something the
    /// runtime cannot honour — a lineup naming the same component
    /// twice, an interpolation whose braces do not close.
    Invalid,
    /// The document could not be read from where it was named.
    ///
    /// Distinct from [`Malformed`](Self::Malformed): the bytes
    /// never arrived, so nothing was parsed. An operator fixes a
    /// path or a permission, not the configuration.
    Unreadable,
}

impl ErrorKind {
    /// A short, stable label for this kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Malformed => "malformed",
            Self::MissingVariable => "missing variable",
            Self::Invalid => "invalid",
            Self::Unreadable => "unreadable",
        }
    }
}

/// A configuration load failure.
#[derive(Debug, Clone)]
pub struct Error {
    kind: ErrorKind,
    message: String,
}

impl Error {
    /// An error of `kind`, described by `message`.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// What kind of failure this is.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The human-readable description.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind.as_str(), self.message)
    }
}

impl std::error::Error for Error {}

/// The result type every loader in this crate returns.
pub type Result<T, E = Error> = std::result::Result<T, E>;
