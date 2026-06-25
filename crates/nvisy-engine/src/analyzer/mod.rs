//! Compile a [`nvisy_core::plan::AnalyzerSpec`] into an
//! [`elide::Analyzer`] per modality at request time.
//!
//! Symmetric with [`crate::anonymizer`]: the spec is pure data
//! (recognizer + enricher + dedup + scope choices); engine walks
//! it and assembles the matching elide runtime values.
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
//! Pattern (bare + context-enhanced) and NER work on any
//! `TextRecognizable` modality (cells, OCR'd text, transcripts).
//! LLM is per-modality typed in elide and requires an `LlmModality`
//! impl, which today exists only for Text + Image.
//!
//! [`Analyzer`]: elide::Analyzer
//! [`AnalyzerSpec`]: nvisy_core::plan::AnalyzerSpec

mod audio;
mod common;
mod image;
mod scope;
mod tabular;
mod text;

pub use self::audio::compile_audio;
pub(crate) use self::common::build_catalog;
pub use self::image::compile_image;
pub use self::tabular::compile_tabular;
pub use self::text::compile_text;
