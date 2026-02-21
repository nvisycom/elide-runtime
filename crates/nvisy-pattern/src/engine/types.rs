//! Public types produced and consumed by the pattern engine.

use nvisy_core::data::{EntityCategory, EntityKind};

/// How the match was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetectionSource {
    /// Matched by a compiled regular expression.
    Regex,
    /// Matched by Aho-Corasick dictionary lookup.
    Dictionary,
}

/// A single match produced by [`PatternEngine::scan_text`](super::PatternEngine::scan_text).
#[derive(Debug, Clone)]
pub struct PatternMatch {
    /// Name of the pattern that produced this match.
    pub pattern_name: String,
    /// Entity category of the match.
    pub category: EntityCategory,
    /// Entity kind of the match.
    pub entity_kind: EntityKind,
    /// Matched text.
    pub value: String,
    /// Byte offset of the match start in the input text.
    pub start: usize,
    /// Byte offset of the match end in the input text.
    pub end: usize,
    /// Confidence score assigned by the pattern definition.
    pub confidence: f64,
    /// Whether the match came from a regex or a dictionary.
    pub source: DetectionSource,
}

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
