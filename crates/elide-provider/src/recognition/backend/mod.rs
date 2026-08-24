//! Deployment-owned backend configuration: which model, which
//! provider, which engine.
//!
//! The wire's `RecognizerParams.{ner,llm}` selects recognizers
//! by name (or the whole lineup); every detail about which
//! recognizers actually run lives here, on the deployment's
//! side. Operators pick model, backend, and (future) credentials
//! at deployment startup; requests only name which of the
//! operator's recognizers to run.
//!
//! Re-exported flat from the parent module; the per-backend split
//! here is organisation, not a path callers should write.

pub(crate) mod llm;
pub(crate) mod ner;
pub(crate) mod ocr;
pub(crate) mod stt;

pub use self::llm::{
    AttachTo, AuthenticatedProvider, LlmConfig, LlmPrompt, LlmRecognizerConfig, LlmSource,
    UnauthenticatedProvider,
};
pub use self::ner::{NerBackend, NerConfig, NerRecognizerConfig};
pub use self::ocr::{OcrBackend, OcrConfig, OcrEnricherConfig};
pub use self::stt::{SttBackend, SttConfig, SttEnricherConfig};
