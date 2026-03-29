//! Validation error type for ontology data structures.

/// A validation error from ontology graph or configuration checks.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ValidationError {
    /// Human-readable description of what failed.
    pub message: String,
}

impl ValidationError {
    /// Create a new validation error.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<validator::ValidationErrors> for ValidationError {
    fn from(err: validator::ValidationErrors) -> Self {
        Self::new(err.to_string())
    }
}

impl From<ValidationError> for nvisy_core::Error {
    fn from(err: ValidationError) -> Self {
        nvisy_core::Error::validation(err.message, "ontology")
    }
}
