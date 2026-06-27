//! Compile a [`nvisy_core::plan::AnalyzerParams`] into an
//! [`elide::detection::Analyzer`] per modality.
//!
//! Symmetric with [`super::anonymizer`]: the spec is pure data
//! (recognizer + enricher + dedup choices); engine walks it and
//! assembles the matching elide runtime values.
//!
//! Scope is **not** per-modality — `elide::recognition::Scope` is
//! modality-free and is built once in [`super::scope`], then
//! attached to the [`Orchestrator`] via [`Orchestrator::with_scope`].
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
//! [`AnalyzerParams`]: nvisy_core::plan::AnalyzerParams
//! [`Orchestrator`]: elide::Orchestrator
//! [`Orchestrator::with_scope`]: elide::Orchestrator::with_scope

mod audio;
mod common;
mod image;
mod tabular;
mod text;

pub(crate) use self::audio::compile_audio;
pub(crate) use self::common::build_catalog;
pub(crate) use self::image::compile_image;
pub(crate) use self::tabular::compile_tabular;
pub(crate) use self::text::compile_text;
