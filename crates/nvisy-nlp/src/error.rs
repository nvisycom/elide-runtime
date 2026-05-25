//! Errors produced by `nvisy-nlp` backends and the composite engine.
//!
//! Backends surface these wrapped in [`CoreError`] via `From<Error>`.
//! Callers that need structured access can downcast through
//! [`CoreError::source`].
//!
//! [`CoreError`]: nvisy_core::Error
//! [`CoreError::source`]: nvisy_core::Error::source

use nvisy_core::{Error as CoreError, ErrorKind};
use nvisy_ontology::primitive::LanguageTag;

/// Result alias for `nvisy-nlp` operations.
///
/// Defaults the error type to [`Error`]; backends and helpers in
/// this crate use this everywhere instead of writing
/// `Result<T, Error>` by hand.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors that can occur during NLP backend construction or inference.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The backend cannot handle the requested language.
    #[error("backend does not support language '{0}'")]
    UnsupportedLanguage(LanguageTag),

    /// Catch-all from a backend implementation.
    #[error("backend error: {0}")]
    Backend(String),
}

impl From<Error> for CoreError {
    fn from(err: Error) -> Self {
        let kind = match &err {
            Error::Backend(_) => ErrorKind::Runtime,
            Error::UnsupportedLanguage(_) => ErrorKind::Validation,
        };
        CoreError::new(kind, err.to_string())
            .with_component("nvisy-nlp")
            .with_source(err)
    }
}
