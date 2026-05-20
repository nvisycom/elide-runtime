//! Errors produced by [`nvisy-nlp`] backends and the composite engine.
//!
//! Backends surface these wrapped in [`nvisy_core::Error`] via
//! `From<NlpError>`. Callers that need structured access can downcast
//! through [`Error::source`](nvisy_core::Error::source).

use std::path::PathBuf;

use nvisy_core::{Error, ErrorKind};
use nvisy_ontology::primitive::LanguageTag;

/// Errors that can occur during NLP backend construction or inference.
#[derive(Debug, thiserror::Error)]
pub enum NlpError {
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

impl From<NlpError> for Error {
    fn from(err: NlpError) -> Self {
        let kind = match &err {
            NlpError::ModelLoad { .. } | NlpError::Tokenizer(_) => ErrorKind::Internal,
            NlpError::Inference(_) | NlpError::Backend(_) => ErrorKind::Runtime,
            NlpError::UnsupportedLanguage(_) => ErrorKind::Validation,
        };
        Error::new(kind, err.to_string())
            .with_component("nvisy-nlp")
            .with_source(err)
    }
}
