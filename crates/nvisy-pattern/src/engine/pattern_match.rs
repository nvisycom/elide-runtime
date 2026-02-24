//! [`PatternMatch`] and [`DetectionSource`] — output types from pattern scanning.

use nvisy_ontology::entity::{EntityCategory, EntityKind};

use crate::patterns::ContextRule;

/// How the match was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetectionSource {
    /// Matched by a compiled regular expression.
    Regex,
    /// Matched by Aho-Corasick dictionary lookup.
    Dictionary,
    /// Injected by the deny list (known sensitive value).
    DenyList,
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
    /// How this match was produced (regex, dictionary, or deny list).
    pub source: DetectionSource,
    /// Optional context rule for span-level co-occurrence scoring.
    pub context: Option<ContextRule>,
}
