//! Error and result types for ontology data structures.

use std::result;

/// An error from ontology graph or configuration checks.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    /// Human-readable description of what failed.
    pub message: String,
}

/// Convenience alias for `std::result::Result<T, Error>`.
pub type Result<T, E = Error> = result::Result<T, E>;

impl Error {
    /// Create a new error.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
