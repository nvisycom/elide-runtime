//! Analyzer plan: serialisable description of how to build an
//! [`elide::detection::Analyzer`] for a request.
//!
//! Symmetric with [`crate::policy`]: where Policy describes
//! redaction governance (which entities to hide and how), the plan
//! describes recognition (which entities to find and how). Both are
//! pure data — engine compiles them into elide runtime values at
//! request time.
//!
//! ## Layout
//!
//! - [`AnalyzerParams`] — top-level: recognizers, enrichers, dedup
//!   pipeline, scope, label catalog.
//! - [`recognizer`] — per-recognizer specs (Pattern, NER, LLM).
//! - [`enricher`] — per-enricher specs (language detection).
//! - [`deduplication`] — fuse / resolve / filter / calibration
//!   strategies.
//! - [`scope`] — language + jurisdiction assertions the caller
//!   passes into recognition.
//!
//! [`elide::detection::Analyzer`]: https://docs.rs/elide/latest/elide/struct.Analyzer.html

mod analyzer;
pub mod deduplication;
pub mod enricher;
pub mod recognizer;
pub mod scope;

pub use self::analyzer::AnalyzerParams;
pub use self::deduplication::{
    CalibrationMap, DeduplicationParams, FusionStrategyParams, ResolutionStrategyParams,
};
pub use self::enricher::{
    EnricherParams, LanguageEnricherParams, OcrBackendParams, OcrEnricherParams, SttBackendParams,
    SttEnricherParams,
};
pub use self::recognizer::{
    LlmBackendParams, LlmRecognizerParams, NerBackendParams, NerRecognizerParams,
    PatternRecognizerParams, RecognizerParams,
};
pub use self::scope::ScopeParams;
