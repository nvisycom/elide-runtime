//! Compiled per-pattern metadata stored inside [`PatternEngine`].
//!
//! [`PatternEngine`]: super::super::PatternEngine

use aho_corasick::AhoCorasick;
use globset::{Glob, GlobSet, GlobSetBuilder};
use nvisy_ontology::entity::{EntityCategory, EntityKind};
use regex::{Regex, RegexSet};

use crate::dictionaries::DictionaryTerm;
use crate::engine::error::PatternEngineError;
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

/// Per-pattern metadata for a glob match. Parallel-indexed with the
/// shared [`GlobSet`] inside [`CompiledBuckets`] — the bucket index
/// returned by `GlobSet::matches` doubles as the index into the
/// engine's `Vec<GlobEntry>`.
#[derive(Debug)]
pub(crate) struct GlobEntry {
    pub pattern_name: String,
    pub category: EntityCategory,
    pub entity_kind: EntityKind,
    pub confidence: f64,
    pub case_sensitive: bool,
    pub context: Option<ContextRule>,
}

/// One compiled pattern, ready for insertion into a
/// [`CompiledBuckets`] via [`CompiledBuckets::insert`].
///
/// The regex variant carries its source string so the builder can
/// fold every entry into a single [`RegexSet`] pre-filter. The glob
/// variant carries its already-compiled [`Glob`] for the same reason
/// — combined into a per-case-sensitivity [`GlobSet`] at finalize
/// time.
///
/// [`PatternCompile::compile`]: crate::patterns::PatternCompile::compile
#[derive(Debug)]
pub(crate) enum CompiledPattern {
    Regex {
        entry: RegexEntry,
        regex_source: String,
    },
    Glob {
        entry: GlobEntry,
        glob: Glob,
    },
    Dictionary(DictEntry),
}

/// Bundle of per-case-sensitivity glob state: parallel-indexed
/// `Vec<GlobEntry>` and the shared [`GlobSet`] that matches against
/// them.
#[derive(Debug)]
pub(in crate::engine) struct GlobBucket {
    pub entries: Vec<GlobEntry>,
    pub set: GlobSet,
}

/// Compiled glob group: case-sensitive and case-insensitive
/// sub-buckets, each with its own [`GlobSet`] so the scan phase can
/// dispatch a single batched match per case-bucket per token.
#[derive(Debug)]
pub(in crate::engine) struct CompiledGlobs {
    pub case_sensitive: GlobBucket,
    pub case_insensitive: GlobBucket,
}

/// Output of [`CompiledBuckets::finish`]: ready-to-scan engine
/// state.
#[derive(Debug)]
pub(in crate::engine) struct CompiledEngine {
    pub regex_set: RegexSet,
    pub regex_entries: Vec<RegexEntry>,
    pub globs: CompiledGlobs,
    pub dict_entries: Vec<DictEntry>,
}

/// Result of compiling a set of patterns: per-kind entry vectors
/// plus the shared [`RegexSet`] / [`GlobSet`] sources collected
/// alongside.
///
/// Built up via [`Self::insert`] (one [`CompiledPattern`] at a time)
/// and finalized via [`Self::finish`], which builds the prefilters
/// and surrenders a [`CompiledEngine`].
#[derive(Debug, Default)]
pub(in crate::engine) struct CompiledBuckets {
    regex_entries: Vec<RegexEntry>,
    regex_sources: Vec<String>,
    cs_glob_entries: Vec<GlobEntry>,
    cs_globs: Vec<Glob>,
    ci_glob_entries: Vec<GlobEntry>,
    ci_globs: Vec<Glob>,
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
            CompiledPattern::Glob { entry, glob } => {
                if entry.case_sensitive {
                    self.cs_glob_entries.push(entry);
                    self.cs_globs.push(glob);
                } else {
                    self.ci_glob_entries.push(entry);
                    self.ci_globs.push(glob);
                }
            }
            CompiledPattern::Dictionary(entry) => self.dict_entries.push(entry),
        }
    }

    /// Build the shared prefilters. Failures wrap the underlying
    /// compiler error in [`PatternEngineError`].
    pub(in crate::engine) fn finish(self) -> Result<CompiledEngine, PatternEngineError> {
        let regex_set =
            RegexSet::new(&self.regex_sources).map_err(PatternEngineError::RegexSetBuild)?;
        Ok(CompiledEngine {
            regex_set,
            regex_entries: self.regex_entries,
            globs: CompiledGlobs {
                case_sensitive: build_glob_bucket(self.cs_glob_entries, self.cs_globs)?,
                case_insensitive: build_glob_bucket(self.ci_glob_entries, self.ci_globs)?,
            },
            dict_entries: self.dict_entries,
        })
    }
}

fn build_glob_bucket(
    entries: Vec<GlobEntry>,
    globs: Vec<Glob>,
) -> Result<GlobBucket, PatternEngineError> {
    let mut builder = GlobSetBuilder::new();
    for glob in globs {
        builder.add(glob);
    }
    let set = builder.build().map_err(PatternEngineError::GlobSetBuild)?;
    Ok(GlobBucket { entries, set })
}
