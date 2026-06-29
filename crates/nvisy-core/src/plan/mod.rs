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
//! - [`enricher`] — per-enricher specs (language detection, OCR,
//!   STT).
//! - [`deduplication`] — calibrate / reconcile / filter strategies.
//! - [`label`] — per-request label catalog selection: builtins by
//!   name + custom inline schemas.
//!
//! Caller-asserted scope lives under [`AnalyzerParams::scope`] —
//! a single [`ScopeParams`] grouping `languages`, `countries`,
//! `labels`, and `label_catalog`. The engine assembles these (plus
//! a server-minted correlation id) into an
//! [`elide::recognition::Scope`] at compile time.
//!
//! [`elide::detection::Analyzer`]: https://docs.rs/elide/latest/elide/struct.Analyzer.html
//! [`elide::recognition::Scope`]: https://docs.rs/elide/latest/elide/recognition/struct.Scope.html

mod analyzer;
pub mod deduplication;
pub mod enricher;
pub mod label;
pub mod recognizer;

pub use self::analyzer::{AnalyzerParams, ScopeParams};
pub use self::deduplication::{DeduplicationParams, MergingStrategyParams, TiebreakerParams};
pub use self::enricher::{
    EnricherParams, LanguageEnricherParams, OcrBackendParams, OcrEnricherParams, SttBackendParams,
    SttEnricherParams,
};
pub use self::label::LabelCatalogParams;
pub use self::recognizer::{
    LlmBackendParams, LlmRecognizerParams, NerBackendParams, NerRecognizerParams,
    PatternRecognizerParams, RecognizerParams,
};
