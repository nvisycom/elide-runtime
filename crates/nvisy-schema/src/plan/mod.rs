//! Authored recognition plan: caller-inlined pattern extras,
//! scope, annotations, and per-request codec knobs.
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
//! - [`AnalyzerParams`]: top-level. Caller-inlined pattern
//!   extras, scope, region annotations, per-request OCR mode.
//! - [`RecognizerParams`]: caller-inlined regex rules and
//!   dictionaries. The bare built-in pattern recognizer is
//!   always attached; NER and LLM recognizers wired via
//!   `Engine::with_ner` / `Engine::with_llm` always run.
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
mod pattern;
mod recognizer;

pub use self::analyzer::{AnalyzerParams, ScopeParams, scope_metadata_is_empty};
pub use self::annotation::AnyAnnotations;
pub use self::pattern::{
    CustomDictionary, CustomDictionaryTerm, CustomPatternContext, CustomPatternRule,
    CustomPatternVariant, MAX_REGEX_SOURCE_LEN,
};
pub use self::recognizer::RecognizerParams;
