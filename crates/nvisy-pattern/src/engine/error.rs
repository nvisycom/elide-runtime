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
    /// A glob pattern string failed to compile.
    #[error("failed to compile glob for pattern '{name}': {source}")]
    GlobCompile {
        name: String,
        source: globset::Error,
    },
    /// A pattern references a dictionary that does not exist.
    #[error("pattern '{name}' references unknown dictionary '{dictionary}'")]
    UnknownDictionary { name: String, dictionary: String },
    /// Failed to build an Aho-Corasick automaton.
    #[error("failed to build Aho-Corasick automaton for dictionary '{name}': {source}")]
    AhoCorasickBuild {
        name: String,
        source: aho_corasick::BuildError,
    },
    /// Failed to build the RegexSet pre-filter.
    #[error("failed to build RegexSet pre-filter: {0}")]
    RegexSetBuild(regex::Error),
    /// Failed to build the GlobSet pre-filter.
    #[error("failed to build GlobSet pre-filter: {0}")]
    GlobSetBuild(globset::Error),
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
/// [`ScanContext::extra_patterns`] entry fails to compile.
///
/// Carries the offending pattern's name plus the underlying
/// [`PatternEngineError`] so callers can decide whether to fail the
/// request or log and continue.
///
/// [`PatternEngine::validate_patterns`]: super::PatternEngine::validate_patterns
/// [`ScanContext::extra_patterns`]: super::filter::ScanContext::extra_patterns
#[derive(Debug, thiserror::Error)]
#[error("extra_pattern '{name}' failed: {source}")]
pub struct ExtraPatternError {
    /// Name of the pattern that failed to compile.
    pub name: String,
    /// The underlying compile error.
    #[source]
    pub source: PatternEngineError,
}
