//! Recognition: which entities to find, and how.
//!
//! Two halves of one story. The backend configuration is the
//! deployment's own — which NER model, which LLM provider, which OCR
//! and STT engines — as serializable types an operator writes once
//! at startup. The rest of this module compiles that configuration
//! into an [`elide::detection::Analyzer`] per modality.
//!
//! They live together because they change together: adding a
//! backend means a config type *and* the code that compiles it, and
//! splitting those across two module trees meant reading two files
//! to follow one backend.
//!
//! Mirrors the crate's redaction side, which does the same for the
//! other direction: where recognition finds entities, redaction
//! hides them.
//!
//! Scope is **not** per-modality: [`Scope`] is modality-free
//! and is built once in the engine's orchestrator builder, then attached to the [`Orchestrator`] via
//! [`Orchestrator::with_scope`].
//!
//! [`Scope`]: elide::recognition::Scope
//!
//! ## Per-modality coverage
//!
//! | Modality | Pattern | NER | LLM |
//! |----------|---------|-----|-----|
//! | Text     | yes     | yes | yes |
//! | Tabular  | yes     | yes | (no upstream `LlmModality` impl) |
//! | Image    | yes     | yes | yes |
//! | Audio    | yes     | yes | (no upstream `LlmModality` impl) |
//!
//! [`Analyzer`]: elide::detection::Analyzer
//! [`Orchestrator`]: elide::Orchestrator
//! [`Orchestrator::with_scope`]: elide::Orchestrator::with_scope

mod backend;
mod enrichers;
mod layer;
mod modality;
mod recognizers;

pub use self::backend::{
    AttachTo, AuthenticatedProvider, LlmConfig, LlmPrompt, LlmRecognizerConfig, LlmSource,
    NerBackend, NerConfig, NerRecognizerConfig, OcrBackend, OcrConfig, OcrEnricherConfig,
    SttBackend, SttConfig, SttEnricherConfig, UnauthenticatedProvider,
};
pub(crate) use self::modality::{compile_audio, compile_image, compile_tabular, compile_text};
