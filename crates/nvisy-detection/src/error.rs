//! Errors produced by recognizers and the detection engine.
//!
//! Recognizers surface their underlying backend errors wrapped in
//! `Error::Recognizer { name, cause }` so the orchestrator can see
//! which recognizer failed without depending on the source type.
//! Conversion into [`nvisy_core::Error`] preserves the source chain.

use nvisy_core::{Error as CoreError, ErrorKind};

/// Result alias for `nvisy-detection` operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors that can occur during recognizer construction or
/// detection-engine execution.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A recognizer failed at runtime. `name` is the recognizer's
    /// `Recognizer::name()` for telemetry; `cause` is the
    /// underlying error stringified at the boundary.
    #[error("recognizer `{name}` failed: {cause}")]
    Recognizer { name: String, cause: String },

    /// Construction failed because a required component was missing.
    #[error("detection engine misconfigured: {0}")]
    Misconfigured(String),
}

impl From<Error> for CoreError {
    fn from(err: Error) -> Self {
        let kind = match &err {
            Error::Recognizer { .. } => ErrorKind::Runtime,
            Error::Misconfigured(_) => ErrorKind::Validation,
        };
        CoreError::new(kind, err.to_string())
            .with_component("nvisy-detection")
            .with_source(err)
    }
}
