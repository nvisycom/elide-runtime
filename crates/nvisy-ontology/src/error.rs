//! Error and result types for ontology data structures.

/// An error from ontology graph or configuration checks.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    /// Human-readable description of what failed.
    pub message: String,
}

/// Convenience alias for `std::result::Result<T, Error>`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// Create a new error.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<validator::ValidationErrors> for Error {
    fn from(err: validator::ValidationErrors) -> Self {
        Self::new(err.to_string())
    }
}

impl From<Error> for nvisy_core::Error {
    fn from(err: Error) -> Self {
        nvisy_core::Error::validation(err.message, "ontology")
    }
}
