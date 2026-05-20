//! Errors produced by `nvisy-nlp` backends and the composite engine.
//!
//! Backends surface these wrapped in [`nvisy_core::Error`] via
//! `From<Error>`. Callers that need structured access can downcast
//! through [`nvisy_core::Error::source`].

use std::path::PathBuf;

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
    /// Failed to load an ONNX model file.
    #[error("failed to load ONNX model at '{}': {cause}", path.display())]
    ModelLoad { path: PathBuf, cause: String },

    /// Failed to load or apply a HuggingFace tokenizer.
    #[error("tokenizer error: {0}")]
    Tokenizer(String),

    /// Model inference itself failed (post-load, post-tokenize).
    #[error("inference failed: {0}")]
    Inference(String),

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
            Error::ModelLoad { .. } | Error::Tokenizer(_) => ErrorKind::Internal,
            Error::Inference(_) | Error::Backend(_) => ErrorKind::Runtime,
            Error::UnsupportedLanguage(_) => ErrorKind::Validation,
        };
        CoreError::new(kind, err.to_string())
            .with_component("nvisy-nlp")
            .with_source(err)
    }
}
