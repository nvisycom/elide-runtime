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
