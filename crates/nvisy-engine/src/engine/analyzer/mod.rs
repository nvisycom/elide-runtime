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
mod catalog;
mod common;
mod image;
mod tabular;
mod text;

use elide::detection::Analyzer;
use elide_core::Error;
use elide_core::modality::audio::Audio;
use elide_core::modality::image::Image;
use elide_core::modality::tabular::Tabular;
use elide_core::modality::text::Text;
use nvisy_core::plan::AnalyzerParams;

pub(crate) use self::catalog::LabelCatalogCompile;

/// Compile a [`nvisy_core::plan::AnalyzerParams`] into a
/// per-modality [`elide::detection::Analyzer`].
///
/// One method per modality — each picks the recognizers and
/// enrichers the modality supports and rejects the rest at
/// compile time (e.g. OCR on text, LLM on tabular).
pub(crate) trait AnalyzerCompile {
    /// Build the text-modality analyzer.
    fn compile_text(&self) -> Result<Analyzer<Text>, Error>;
    /// Build the tabular-modality analyzer.
    fn compile_tabular(&self) -> Result<Analyzer<Tabular>, Error>;
    /// Build the image-modality analyzer.
    fn compile_image(&self) -> Result<Analyzer<Image>, Error>;
    /// Build the audio-modality analyzer.
    fn compile_audio(&self) -> Result<Analyzer<Audio>, Error>;
}

impl AnalyzerCompile for AnalyzerParams {
    fn compile_text(&self) -> Result<Analyzer<Text>, Error> {
        self::text::compile(self)
    }

    fn compile_tabular(&self) -> Result<Analyzer<Tabular>, Error> {
        self::tabular::compile(self)
    }

    fn compile_image(&self) -> Result<Analyzer<Image>, Error> {
        self::image::compile(self)
    }

    fn compile_audio(&self) -> Result<Analyzer<Audio>, Error> {
        self::audio::compile(self)
    }
}
