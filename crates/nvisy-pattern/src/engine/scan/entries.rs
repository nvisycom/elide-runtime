//! Compiled per-pattern metadata stored inside [`PatternEngine`].
//!
//! [`PatternEngine`]: super::PatternEngine

use aho_corasick::AhoCorasick;
use nvisy_ontology::entity::{EntityCategory, EntityKind};
use regex::Regex;

use crate::dictionaries::DictionaryTerm;
use crate::patterns::{ContextRule, DictionaryConfidence};

/// Metadata stored alongside each compiled regex.
#[derive(Debug)]
pub(crate) struct RegexEntry {
    pub pattern_name: String,
    pub category: EntityCategory,
    pub entity_kind: EntityKind,
    pub confidence: f64,
    pub validator_name: Option<String>,
    pub regex: Regex,
    pub context: Option<ContextRule>,
}

/// Metadata stored alongside each compiled Aho-Corasick automaton.
#[derive(Debug)]
pub(crate) struct DictEntry {
    pub pattern_name: String,
    pub category: EntityCategory,
    pub entity_kind: EntityKind,
    pub confidence: DictionaryConfidence,
    pub automaton: AhoCorasick,
    /// Terms keyed by Aho-Corasick pattern id.
    pub terms: Vec<DictionaryTerm>,
    pub context: Option<ContextRule>,
}

impl DictEntry {
    /// Resolve the confidence for the entry at `pattern_index`.
    pub(in crate::engine) fn resolve_confidence(&self, pattern_index: usize) -> f64 {
        let col = self
            .terms
            .get(pattern_index)
            .and_then(|t| t.column)
            .unwrap_or(0) as usize;
        self.confidence.resolve(col)
    }
}

/// One compiled pattern, ready for insertion into the engine.
///
/// Produced by [`PatternCompile::compile`]; consumed by the
/// [`PatternEngineBuilder`] when assembling the final engine. The
/// regex variant also carries the effective regex string so the
/// builder can fold every entry into a single `RegexSet` pre-filter.
///
/// [`PatternCompile::compile`]: crate::patterns::PatternCompile::compile
/// [`PatternEngineBuilder`]: super::super::PatternEngineBuilder
#[derive(Debug)]
pub(crate) enum CompiledPattern {
    /// A compiled regex pattern plus the original regex source —
    /// the source is needed to populate the engine's `RegexSet`.
    Regex {
        entry: RegexEntry,
        regex_source: String,
    },
    /// A compiled dictionary pattern.
    Dictionary(DictEntry),
}
