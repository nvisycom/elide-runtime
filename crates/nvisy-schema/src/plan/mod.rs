//! Authored recognition plan: recognizers, enrichers, dedup, scope.
//!
//! Serialisable description of how to build an analyzer for a
//! request. Symmetric with [`policy`]. Where policy describes
//! redaction governance (which entities to hide and how), the
//! plan describes recognition (which entities to find and how).
//! Both are pure data; the engine compiles them into elide
//! runtime values at request time.
//!
//! ## Layout
//!
//! - [`AnalyzerParams`]: top-level. Recognizers, enrichers,
//!   dedup pipeline, scope.
//! - [`RecognizerParams`]: per-recognizer specs (Pattern, NER,
//!   LLM).
//! - [`EnricherParams`]: per-enricher specs (language detection,
//!   OCR, STT).
//! - [`DeduplicationParams`]: calibrate / reconcile / filter
//!   strategies.
//! - [`AnyAnnotations`]: per-modality region annotations
//!   (inclusions / exclusions).
//!
//! Caller-asserted scope lives under [`AnalyzerParams::scope`],
//! a [`ScopeParams`] carrying `languages`, `countries`, and
//! elide's own `ScopeMetadata` block (tags, purpose, audience).
//! The engine assembles these (plus a server-minted correlation
//! id and the policy-derived label catalog) into an
//! `elide::recognition::Scope` at compile time.
//!
//! [`policy`]: crate::policy

mod analyzer;
mod annotation;
mod deduplication;
mod enricher;
mod pattern;
mod recognizer;

pub use self::analyzer::{AnalyzerParams, ScopeParams, scope_metadata_is_empty};
pub use self::annotation::AnyAnnotations;
pub use self::deduplication::{DeduplicationParams, MergingStrategyParams, TiebreakerParams};
pub use self::enricher::{
    EnricherParams, LanguageEnricherParams, OcrBackendParams, OcrEnricherParams, SttBackendParams,
    SttEnricherParams,
};
pub use self::pattern::{
    CustomDictionary, CustomDictionaryTerm, CustomPatternContext, CustomPatternRule,
    CustomPatternVariant, MAX_REGEX_SOURCE_LEN,
};
pub use self::recognizer::{PatternRecognizerParams, ProviderSelection, RecognizerParams};
