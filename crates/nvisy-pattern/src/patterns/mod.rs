//! Built-in detection patterns.
//!
//! Each pattern is a JSON file under `assets/patterns/` that describes how
//! to detect a single entity type. Files are embedded at compile time with
//! `include_dir!` and auto-discovered by [`PatternRegistry::load_builtins`].
//!
//! # Key types
//!
//! - [`Pattern`]: trait implemented by every pattern.
//! - [`JsonPattern`]: concrete implementation deserialized from JSON.
//! - [`RuntimePattern`]: programmatic pattern for per-call extras.
//! - [`MatchSource`]: regex, glob, or dictionary matching.
//! - [`ContextRule`]: optional co-occurrence keywords for confidence boosting.
//! - [`PatternRegistry`]: sorted collection with O(log n) lookup.
//! - [`JsonPatternWarning`]: non-fatal load-time diagnostics.

mod context_rule;
mod json_pattern;
mod pattern;
mod pattern_error;
mod pattern_metadata;
mod pattern_registry;
mod runtime_pattern;

pub(crate) use self::context_rule::ContextRule;
pub(crate) use self::json_pattern::{JsonPattern, JsonPatternWarning};
pub use self::pattern::{
    DictionaryConfidence, DictionaryPattern, GlobPattern, MatchSource, RegexPattern,
};
pub(crate) use self::pattern::{Pattern, PatternCompile};
pub(crate) use self::pattern_error::PatternLoadError;
pub(crate) use self::pattern_registry::{PatternRegistry, builtin_registry};
pub use self::runtime_pattern::RuntimePattern;
