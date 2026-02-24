//! Errors produced during [`PatternEngine`](super::PatternEngine) construction.

/// Errors that can occur while building a [`PatternEngine`](super::PatternEngine).
#[derive(Debug, thiserror::Error)]
pub enum PatternEngineError {
    /// A regex pattern string failed to compile.
    #[error("failed to compile regex for pattern '{name}': {source}")]
    RegexCompile {
        name: String,
        source: regex::Error,
    },
    /// A pattern references a dictionary that does not exist.
    #[error("pattern '{name}' references unknown dictionary '{dictionary}'")]
    UnknownDictionary {
        name: String,
        dictionary: String,
    },
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
