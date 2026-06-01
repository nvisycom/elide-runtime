//! Compiled per-pattern metadata stored inside [`PatternEngine`].
//!
//! [`PatternEngine`]: super::super::PatternEngine

use aho_corasick::AhoCorasick;
use nvisy_ontology::entity::EntityKind;
use regex::{Regex, RegexSet};

use crate::dictionaries::DictionaryTerm;
use crate::engine::error::PatternEngineError;
use crate::patterns::{ContextRule, DictionaryConfidence};

/// Metadata stored alongside each compiled regex.
#[derive(Debug)]
pub(crate) struct RegexEntry {
    pub pattern_name: String,
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

/// One compiled pattern, ready for insertion into a
/// [`CompiledBuckets`] via [`CompiledBuckets::insert`].
///
/// The regex variant carries its source string so the builder can
/// fold every entry into a single [`RegexSet`] pre-filter.
///
/// [`PatternCompile::compile`]: crate::patterns::PatternCompile::compile
#[derive(Debug)]
pub(crate) enum CompiledPattern {
    Regex {
        entry: RegexEntry,
        regex_source: String,
    },
    Dictionary(DictEntry),
}

/// Output of [`CompiledBuckets::finish`]: ready-to-scan engine
/// state.
#[derive(Debug)]
pub(in crate::engine) struct CompiledEngine {
    pub regex_set: RegexSet,
    pub regex_entries: Vec<RegexEntry>,
    pub dict_entries: Vec<DictEntry>,
}

/// Result of compiling a set of patterns: per-kind entry vectors
/// plus the shared [`RegexSet`] sources collected alongside.
///
/// Built up via [`Self::insert`] (one [`CompiledPattern`] at a time)
/// and finalized via [`Self::finish`], which builds the prefilter
/// and surrenders a [`CompiledEngine`].
#[derive(Debug, Default)]
pub(in crate::engine) struct CompiledBuckets {
    regex_entries: Vec<RegexEntry>,
    regex_sources: Vec<String>,
    dict_entries: Vec<DictEntry>,
}

impl CompiledBuckets {
    /// Route a freshly compiled pattern into the matching bucket.
    pub(in crate::engine) fn insert(&mut self, pattern: CompiledPattern) {
        match pattern {
            CompiledPattern::Regex {
                entry,
                regex_source,
            } => {
                self.regex_entries.push(entry);
                self.regex_sources.push(regex_source);
            }
            CompiledPattern::Dictionary(entry) => self.dict_entries.push(entry),
        }
    }

    /// Build the shared prefilter. Failures wrap the underlying
    /// compiler error in [`PatternEngineError`].
    pub(in crate::engine) fn finish(self) -> Result<CompiledEngine, PatternEngineError> {
        let regex_set =
            RegexSet::new(&self.regex_sources).map_err(PatternEngineError::RegexSetBuild)?;
        Ok(CompiledEngine {
            regex_set,
            regex_entries: self.regex_entries,
            dict_entries: self.dict_entries,
        })
    }
}
