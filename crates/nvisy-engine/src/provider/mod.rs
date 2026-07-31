//! Deployment-owned recognizer provider configuration.
//!
//! The wire's `RecognizerParams.{ner,llm}` selects recognizers
//! by name (or the whole lineup); every detail about which
//! recognizers actually run lives here, on the deployment's
//! side. Operators pick model, backend, and (future) credentials
//! at deployment startup; requests only name which of the
//! operator's recognizers to run.

pub(crate) mod llm;
pub(crate) mod ner;

pub use self::llm::{
    AttachTo, AuthenticatedProvider, LlmConfig, LlmPrompt, LlmRecognizer, LlmSource,
    UnauthenticatedProvider,
};
pub use self::ner::{NerBackend, NerConfig, NerRecognizer};
