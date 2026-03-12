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
//! - [`MatchSource`]: whether matching is regex-based or dictionary-based.
//! - [`ContextRule`]: optional co-occurrence keywords for confidence boosting.
//! - [`PatternRegistry`]: sorted collection with O(log n) lookup.
//! - [`JsonPatternWarning`]: non-fatal load-time diagnostics.

mod context_rule;
mod json_pattern;
mod pattern;
mod pattern_error;
mod pattern_registry;

pub use self::context_rule::ContextRule;
pub use self::json_pattern::{JsonPattern, JsonPatternWarning};
pub use self::pattern::{BoxPattern, DictionaryConfidence, MatchSource, Pattern};
pub(crate) use self::pattern_error::PatternLoadError;
pub use self::pattern_registry::{PatternRegistry, builtin_registry};
