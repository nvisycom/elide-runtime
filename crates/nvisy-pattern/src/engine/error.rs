//! Errors produced during [`PatternEngine`] construction.
//!
//! [`PatternEngine`]: super::PatternEngine

use nvisy_core::{Error, ErrorKind};

/// Errors that can occur while building a [`PatternEngine`].
///
/// Built engines surface these wrapped in [`CoreError`] via
/// `From<PatternEngineError>`. Callers that need structured access can
/// downcast through [`Error::source`].
///
/// [`CoreError`]: nvisy_core::Error
/// [`PatternEngine`]: super::PatternEngine
/// [`Error::source`]: nvisy_core::Error::source
#[derive(Debug, thiserror::Error)]
pub enum PatternEngineError {
    /// A regex pattern string failed to compile.
    #[error("failed to compile regex for pattern '{name}': {source}")]
    RegexCompile { name: String, source: regex::Error },
    /// A pattern references a dictionary that does not exist.
    #[error("pattern '{name}' references unknown dictionary '{dictionary}'")]
    UnknownDictionary { name: String, dictionary: String },
    /// Failed to build an Aho-Corasick automaton.
    #[error("failed to build Aho-Corasick automaton for dictionary '{name}': {source}")]
    AhoCorasickBuild {
        name: String,
        source: aho_corasick::BuildError,
    },
    /// A dictionary pattern declared per-column confidence but the
    /// referenced dictionary has case-insensitive duplicate terms
    /// across different columns — the resolved confidence is ambiguous.
    ///
    /// Fix by deduplicating the dictionary (drop the rogue cell or
    /// move it to the matching column) or by setting the pattern's
    /// `case_sensitive: true` so case-distinct cells no longer collide.
    #[error(
        "pattern '{name}' has ambiguous per-column confidence: dictionary '{dictionary}' \
         term '{term}' appears in columns {columns:?} under case-insensitive matching"
    )]
    AmbiguousDictionaryConfidence {
        name: String,
        dictionary: String,
        term: String,
        columns: Vec<u32>,
    },
    /// Failed to build the RegexSet pre-filter.
    #[error("failed to build RegexSet pre-filter: {0}")]
    RegexSetBuild(regex::Error),
}

impl From<PatternEngineError> for Error {
    fn from(err: PatternEngineError) -> Self {
        Error::new(ErrorKind::Validation, err.to_string())
            .with_component("nvisy-pattern::engine")
            .with_source(err)
    }
}

/// Per-extra compile error surfaced by
/// [`PatternEngine::validate_patterns`] when a
/// [`PatternContext::extra_patterns`] entry fails to compile.
///
/// Carries the offending pattern's name plus the underlying
/// [`PatternEngineError`] so callers can decide whether to fail the
/// request or log and continue.
///
/// [`PatternEngine::validate_patterns`]: super::PatternEngine::validate_patterns
/// [`PatternContext::extra_patterns`]: super::filter::PatternContext::extra_patterns
#[derive(Debug, thiserror::Error)]
#[error("extra_pattern '{name}' failed: {source}")]
pub struct ExtraPatternError {
    /// Name of the pattern that failed to compile.
    pub name: String,
    /// The underlying compile error.
    #[source]
    pub source: PatternEngineError,
}
