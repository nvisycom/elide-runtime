//! Recognition: which entities to find, and how.
//!
//! Split by what a component does. A **recognizer** produces
//! entities; an **enricher** produces the context recognizers read,
//! running before them to stamp a language hint, OCR'd text layout,
//! or audio transcript segments onto the request.
//!
//! Each group holds one module per backend, carrying the
//! deployment's own configuration — which NER model, which LLM
//! provider, which OCR and STT engines — beside the `compile` step
//! that turns those lineups into an [`elide::detection::Analyzer`]
//! per modality. Config and compile live together because they
//! change together: adding a backend means a config type *and* the
//! code that reads it.
//!
//! Mirrors the crate's redaction side, which does the same for the
//! other direction: where recognition finds entities, redaction
//! hides them.
//!
//! Scope is **not** per-modality: [`Scope`] is modality-free and is
//! built once in the orchestrator builder, then attached to the
//! [`Orchestrator`] via [`Orchestrator::with_scope`].
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

mod component;
mod enrichers;
mod layer;
mod modality;
mod recognizers;

pub use self::component::{Backend, Component};
pub use self::enrichers::{OcrBackend, OcrConfig, SttBackend, SttConfig};
pub(crate) use self::modality::{compile_audio, compile_image, compile_tabular, compile_text};
pub use self::recognizers::{
    AttachTo, AuthenticatedProvider, LlmBackend, LlmConfig, LlmSource, NerBackend, NerConfig,
    UnauthenticatedProvider,
};
