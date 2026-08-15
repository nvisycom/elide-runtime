//! Compile the deployment's recognizer + enricher lineups into
//! an [`elide::detection::Analyzer`] per modality.
//!
//! Symmetric with [`crate::anonymizer`]: the engine holds the
//! deployment-owned recognizer and enricher configuration
//! (`NerConfig` / `LlmConfig` / OCR / STT); compile walks it.
//!
//! Scope is **not** per-modality: [`Scope`] is modality-free
//! and is built once in [`crate::pipeline`]'s orchestrator
//! builder, then attached to the [`Orchestrator`] via
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

mod catalog;
mod enrichers;
mod layer;
mod modality;
mod recognizers;

pub(crate) use self::catalog::compile_catalog;
#[cfg(feature = "internal_audio")]
pub(crate) use self::modality::compile_audio;
#[cfg(feature = "internal_image")]
pub(crate) use self::modality::compile_image;
#[cfg(feature = "internal_tabular")]
pub(crate) use self::modality::compile_tabular;
pub(crate) use self::modality::compile_text;
