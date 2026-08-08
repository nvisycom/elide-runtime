//! Compile a [`nvisy_schema::plan::AnalyzerParams`] into an
//! [`elide::detection::Analyzer`] per modality.
//!
//! Symmetric with [`crate::anonymizer`]: the spec is pure data
//! (recognizer + enricher + dedup choices); engine walks it and
//! assembles the matching elide runtime values.
//!
//! Scope is **not** per-modality — [`Scope`] is modality-free
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
//! [`AnalyzerParams`]: nvisy_schema::plan::AnalyzerParams
//! [`Orchestrator`]: elide::Orchestrator
//! [`Orchestrator::with_scope`]: elide::Orchestrator::with_scope

#[cfg(feature = "internal_audio")]
mod audio;
mod catalog;
mod enricher;
#[cfg(feature = "internal_image")]
mod image;
mod layer;
mod recognizer;
#[cfg(feature = "internal_tabular")]
mod tabular;
mod text;

use elide::detection::Analyzer;
use elide_core::Result;
#[cfg(feature = "internal_audio")]
use elide_core::modality::audio::Audio;
#[cfg(feature = "internal_image")]
use elide_core::modality::image::Image;
#[cfg(feature = "internal_tabular")]
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use nvisy_schema::plan::AnalyzerParams;

pub(crate) use self::catalog::{compile_catalog, synthetic_group_tag};
pub use self::recognizer::PatternGuardrails;
use crate::provider::llm::LlmConfig;
use crate::provider::ner::NerConfig;

/// Compile a [`nvisy_schema::plan::AnalyzerParams`] into a
/// per-modality [`elide::detection::Analyzer`].
///
/// One method per modality; each picks the recognizers and
/// enrichers the modality supports and rejects the rest at
/// compile time (e.g. OCR on text, LLM on tabular). Every
/// compile fn consults the deployment [`NerConfig`] when the
/// request's `recognizers.ner` selects any recognizer; text
/// and image also consult [`LlmConfig`] when
/// `recognizers.llm` does. Every method consults
/// [`PatternGuardrails`] when the pattern recognizer is enabled.
///
/// Non-text methods are gated on their modality's feature.
pub(crate) trait AnalyzerCompile {
    /// Build the text-modality analyzer.
    fn compile_text(
        &self,
        ner: &NerConfig,
        llm: &LlmConfig,
        guardrails: &PatternGuardrails,
    ) -> Result<Analyzer<Text>>;
    /// Build the tabular-modality analyzer.
    #[cfg(feature = "internal_tabular")]
    fn compile_tabular(
        &self,
        ner: &NerConfig,
        guardrails: &PatternGuardrails,
    ) -> Result<Analyzer<Tabular>>;
    /// Build the image-modality analyzer.
    #[cfg(feature = "internal_image")]
    fn compile_image(
        &self,
        ner: &NerConfig,
        llm: &LlmConfig,
        guardrails: &PatternGuardrails,
    ) -> Result<Analyzer<Image>>;
    /// Build the audio-modality analyzer.
    #[cfg(feature = "internal_audio")]
    fn compile_audio(
        &self,
        ner: &NerConfig,
        guardrails: &PatternGuardrails,
    ) -> Result<Analyzer<Audio>>;
}

impl AnalyzerCompile for AnalyzerParams {
    fn compile_text(
        &self,
        ner: &NerConfig,
        llm: &LlmConfig,
        guardrails: &PatternGuardrails,
    ) -> Result<Analyzer<Text>> {
        self::text::compile(self, ner, llm, guardrails)
    }

    #[cfg(feature = "internal_tabular")]
    fn compile_tabular(
        &self,
        ner: &NerConfig,
        guardrails: &PatternGuardrails,
    ) -> Result<Analyzer<Tabular>> {
        self::tabular::compile(self, ner, guardrails)
    }

    #[cfg(feature = "internal_image")]
    fn compile_image(
        &self,
        ner: &NerConfig,
        llm: &LlmConfig,
        guardrails: &PatternGuardrails,
    ) -> Result<Analyzer<Image>> {
        self::image::compile(self, ner, llm, guardrails)
    }

    #[cfg(feature = "internal_audio")]
    fn compile_audio(
        &self,
        ner: &NerConfig,
        guardrails: &PatternGuardrails,
    ) -> Result<Analyzer<Audio>> {
        self::audio::compile(self, ner, guardrails)
    }
}
