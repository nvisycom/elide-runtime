//! Compile a [`nvisy_schema::plan::AnalyzerParams`] into an
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
//! [`AnalyzerParams`]: nvisy_schema::plan::AnalyzerParams
//! [`Orchestrator`]: elide::Orchestrator
//! [`Orchestrator::with_scope`]: elide::Orchestrator::with_scope

#[cfg(feature = "internal_audio")]
mod audio;
mod catalog;
mod common;
#[cfg(feature = "internal_image")]
mod image;
#[cfg(feature = "internal_tabular")]
mod tabular;
mod text;

use elide::detection::Analyzer;
use elide_core::Error;
#[cfg(feature = "internal_audio")]
use elide_core::modality::audio::Audio;
#[cfg(feature = "internal_image")]
use elide_core::modality::image::Image;
#[cfg(feature = "internal_tabular")]
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use nvisy_core::llm::LlmConfig;
use nvisy_core::ner::NerConfig;
use nvisy_schema::plan::AnalyzerParams;

pub(crate) use self::catalog::LabelCatalogCompile;

/// Compile a [`nvisy_schema::plan::AnalyzerParams`] into a
/// per-modality [`elide::detection::Analyzer`].
///
/// One method per modality; each picks the recognizers and
/// enrichers the modality supports and rejects the rest at
/// compile time (e.g. OCR on text, LLM on tabular). Every
/// compile fn consults the deployment [`NerConfig`] when the
/// request toggles `recognizers.ner = true`; text and image
/// also consult [`LlmConfig`] when `recognizers.llm = true`.
///
/// Non-text methods are gated on their modality's feature.
pub(crate) trait AnalyzerCompile {
    /// Build the text-modality analyzer.
    fn compile_text(&self, ner: &NerConfig, llm: &LlmConfig) -> Result<Analyzer<Text>, Error>;
    /// Build the tabular-modality analyzer.
    #[cfg(feature = "internal_tabular")]
    fn compile_tabular(&self, ner: &NerConfig) -> Result<Analyzer<Tabular>, Error>;
    /// Build the image-modality analyzer.
    #[cfg(feature = "internal_image")]
    fn compile_image(&self, ner: &NerConfig, llm: &LlmConfig) -> Result<Analyzer<Image>, Error>;
    /// Build the audio-modality analyzer.
    #[cfg(feature = "internal_audio")]
    fn compile_audio(&self, ner: &NerConfig) -> Result<Analyzer<Audio>, Error>;
}

impl AnalyzerCompile for AnalyzerParams {
    fn compile_text(&self, ner: &NerConfig, llm: &LlmConfig) -> Result<Analyzer<Text>, Error> {
        self::text::compile(self, ner, llm)
    }

    #[cfg(feature = "internal_tabular")]
    fn compile_tabular(&self, ner: &NerConfig) -> Result<Analyzer<Tabular>, Error> {
        self::tabular::compile(self, ner)
    }

    #[cfg(feature = "internal_image")]
    fn compile_image(&self, ner: &NerConfig, llm: &LlmConfig) -> Result<Analyzer<Image>, Error> {
        self::image::compile(self, ner, llm)
    }

    #[cfg(feature = "internal_audio")]
    fn compile_audio(&self, ner: &NerConfig) -> Result<Analyzer<Audio>, Error> {
        self::audio::compile(self, ner)
    }
}
