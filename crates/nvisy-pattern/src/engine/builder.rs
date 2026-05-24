//! [`PatternEngineBuilder`]: configures and compiles a [`PatternEngine`].

use std::path::{Path, PathBuf};

use regex::RegexSet;

use super::PatternEngine;
use super::error::PatternEngineError;
use super::pattern_filter::PatternFilter;
use super::scan::entries::CompiledPattern;
use crate::dictionaries::{self, DictionaryRegistry};
use crate::patterns::{MatchSource, Pattern, PatternCompile, PatternRegistry};
use crate::validators::ValidatorResolver;

const TARGET: &str = "nvisy_pattern::engine";

/// Builder for [`PatternEngine`].
///
/// By default all built-in patterns are included. Use
/// [`with_patterns`] to restrict to a subset by name,
/// [`with_dictionaries`] to restrict which backing dictionaries
/// participate (only affects dictionary-backed patterns),
/// [`with_filter`] to narrow by metadata tags, and
/// [`with_pattern_dir`] / [`with_dictionary_dir`] to layer
/// user-supplied patterns and dictionaries on top of the
/// built-ins from filesystem paths.
///
/// [`with_patterns`]: Self::with_patterns
/// [`with_dictionaries`]: Self::with_dictionaries
/// [`with_filter`]: Self::with_filter
/// [`with_pattern_dir`]: Self::with_pattern_dir
/// [`with_dictionary_dir`]: Self::with_dictionary_dir
#[derive(Default)]
pub struct PatternEngineBuilder {
    pattern_names: Option<Vec<String>>,
    dictionary_names: Option<Vec<String>>,
    confidence_threshold: f64,
    filter: Option<PatternFilter>,
    extra_pattern_dirs: Vec<PathBuf>,
    extra_dictionary_dirs: Vec<PathBuf>,
}

impl PatternEngineBuilder {
    /// Restrict the engine to the named patterns only.
    ///
    /// If not called (or called with an empty slice), all built-in
    /// patterns are included.
    pub fn with_patterns(mut self, names: &[impl AsRef<str>]) -> Self {
        if !names.is_empty() {
            self.pattern_names = Some(names.iter().map(|n| n.as_ref().to_owned()).collect());
        }
        self
    }

    /// Restrict the engine to dictionary-backed patterns whose
    /// underlying dictionary name is in `names`.
    ///
    /// Has no effect on regex patterns — they're included on the
    /// strength of [`with_patterns`] and [`with_filter`] alone. If
    /// not called (or called with an empty slice), every backing
    /// dictionary is permitted.
    ///
    /// [`with_patterns`]: Self::with_patterns
    /// [`with_filter`]: Self::with_filter
    pub fn with_dictionaries(mut self, names: &[impl AsRef<str>]) -> Self {
        if !names.is_empty() {
            self.dictionary_names = Some(names.iter().map(|n| n.as_ref().to_owned()).collect());
        }
        self
    }

    /// Set the minimum confidence score for matches.
    ///
    /// Matches with confidence below this value are discarded during
    /// [`scan_entities`]. Defaults to `0.0`.
    ///
    /// [`scan_entities`]: PatternEngine::scan_entities
    pub fn with_confidence_threshold(mut self, threshold: f64) -> Self {
        self.confidence_threshold = threshold;
        self
    }

    /// Narrow the active pattern set by metadata tags.
    ///
    /// Applies to both regex and dictionary-backed patterns. A pattern
    /// is included only when its metadata satisfies every non-empty
    /// constraint in `filter`. A pattern with no tags on a particular
    /// axis is treated as **universal** on that axis (it passes any
    /// filter for that field). Dictionary-backed patterns whose own
    /// metadata is empty on an axis fall through to the backing
    /// dictionary's sidecar metadata for that axis.
    pub fn with_filter(mut self, filter: PatternFilter) -> Self {
        if !filter.is_unconstrained() {
            self.filter = Some(filter);
        }
        self
    }

    /// Layer user-supplied patterns from `dir` on top of the built-ins.
    ///
    /// Recurses into subdirectories. Each `.json` file is parsed as a
    /// pattern definition; non-JSON files are logged and skipped. May
    /// be called multiple times to load several directories — each
    /// invocation appends. External patterns are unioned with the
    /// built-ins; duplicate names overwrite the built-in entry.
    pub fn with_pattern_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.extra_pattern_dirs.push(dir.as_ref().to_owned());
        self
    }

    /// Layer user-supplied dictionaries from `dir` on top of the
    /// built-ins.
    ///
    /// Recurses into subdirectories. Each `.txt` or `.csv` file is
    /// loaded as a dictionary; an optional sibling `<stem>.json`
    /// sidecar supplies metadata (language/industry/region/
    /// compliance tags). May be called multiple times to load
    /// several directories — each invocation appends. External
    /// dictionaries are unioned with the built-ins; duplicate names
    /// overwrite the built-in entry.
    pub fn with_dictionary_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.extra_dictionary_dirs.push(dir.as_ref().to_owned());
        self
    }

    /// Compile all selected patterns and build the engine.
    ///
    /// The per-pattern compilation (regex compile, dictionary lookup,
    /// Aho-Corasick automaton build) lives on the crate-private
    /// `PatternCompile::compile` and `DictionaryCompile::build_automaton`;
    /// this method only orchestrates the filtering (name, tag,
    /// dictionary allowlist) and collects the results.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if a regex fails to compile, a
    /// referenced dictionary is missing, or the Aho-Corasick automaton
    /// cannot be built.
    ///
    /// [`Error`]: nvisy_core::Error
    #[tracing::instrument(target = TARGET, name = "PatternEngine::build", skip(self))]
    pub fn build(self) -> nvisy_core::Result<PatternEngine> {
        let builtin_patterns = crate::patterns::builtin_registry();
        let builtin_dicts = dictionaries::builtin_registry();

        // Load any user-supplied dirs into freshly owned overlay
        // registries; the built-ins stay shared via static reference.
        let mut extra_patterns = PatternRegistry::new();
        for dir in &self.extra_pattern_dirs {
            extra_patterns.load_dir(dir)?;
        }
        let mut extra_dicts = DictionaryRegistry::new();
        for dir in &self.extra_dictionary_dirs {
            extra_dicts.load_dir(dir)?;
        }

        // Patterns to consider, in the order: extras first (so a
        // user-supplied name wins the `pat_lookup` allowlist tie),
        // then built-ins.
        let pat_lookup = |name: &str| -> Option<&dyn Pattern> {
            extra_patterns
                .get(name)
                .or_else(|| builtin_patterns.get(name))
        };

        let active: Vec<&dyn Pattern> = match &self.pattern_names {
            Some(names) => names.iter().filter_map(|n| pat_lookup(n)).collect(),
            None => extra_patterns
                .iter()
                .chain(
                    builtin_patterns
                        .iter()
                        .filter(|p| extra_patterns.get(p.name()).is_none()),
                )
                .collect(),
        };

        // Dictionary lookup also prefers extras over built-ins.
        let dict_lookup = |name: &str| -> Option<&dyn crate::dictionaries::Dictionary> {
            extra_dicts.get(name).or_else(|| builtin_dicts.get(name))
        };

        let mut regex_entries = Vec::new();
        let mut regex_strings = Vec::new();
        let mut glob_entries = Vec::new();
        let mut dict_entries = Vec::new();

        for p in &active {
            if let Some(ref filter) = self.filter
                && !pattern_matches_filter(*p, filter, &dict_lookup)
            {
                tracing::trace!(
                    target: TARGET,
                    pattern = p.name(),
                    "skipped by pattern filter",
                );
                continue;
            }

            if let Some(ref allowed) = self.dictionary_names
                && let MatchSource::Dictionary(dp) = p.match_source()
                && !allowed.iter().any(|n| n == &dp.name)
            {
                tracing::trace!(
                    target: TARGET,
                    pattern = p.name(),
                    dictionary = %dp.name,
                    "skipped by dictionary allowlist",
                );
                continue;
            }

            match p.compile_with(&dict_lookup)? {
                Some(CompiledPattern::Regex {
                    entry,
                    regex_source,
                }) => {
                    regex_strings.push(regex_source);
                    regex_entries.push(entry);
                }
                Some(CompiledPattern::Glob(entry)) => {
                    glob_entries.push(entry);
                }
                Some(CompiledPattern::Dictionary(entry)) => {
                    dict_entries.push(entry);
                }
                None => {
                    tracing::trace!(
                        target: TARGET,
                        pattern = p.name(),
                        "skipped: compiled to no-op (e.g. empty dictionary)",
                    );
                }
            }
        }

        let regex_set = RegexSet::new(&regex_strings).map_err(PatternEngineError::RegexSetBuild)?;

        let validators = ValidatorResolver::builtins();

        tracing::debug!(
            target: TARGET,
            regex_count = regex_entries.len(),
            glob_count = glob_entries.len(),
            dict_count = dict_entries.len(),
            "PatternEngine built",
        );

        Ok(PatternEngine {
            regex_set,
            regex_entries,
            glob_entries,
            dict_entries,
            validators,
            confidence_threshold: self.confidence_threshold,
        })
    }
}

/// Whether `p`'s metadata satisfies every non-empty constraint in
/// `filter`. Per axis the rule is:
///
/// - filter field empty → unconstrained (always passes)
/// - pattern field non-empty → pattern's tags decide
/// - pattern field empty AND dictionary-backed → fall through to the
///   backing dictionary's sidecar metadata for that axis (Pattern
///   wins; dictionary fills in)
/// - all sources empty → universal on that axis (passes)
///
/// Within a non-empty source field, tags must overlap with the
/// filter (OR within field). Across fields the test is AND.
fn pattern_matches_filter<'a, F>(p: &dyn Pattern, filter: &PatternFilter, dict_lookup: &F) -> bool
where
    F: Fn(&str) -> Option<&'a dyn crate::dictionaries::Dictionary>,
{
    // Fetch the backing dictionary's metadata once if this is a
    // dictionary-backed pattern — used as the fall-through source
    // for any axis the pattern itself doesn't tag.
    let dict_md = if let MatchSource::Dictionary(dp) = p.match_source() {
        dict_lookup(&dp.name).map(|d| d.metadata())
    } else {
        None
    };

    let md = p.metadata();

    // For each axis, the effective tag set is: the pattern's tags if
    // non-empty, else the dictionary's tags. Empty → universal.
    let langs = if !md.languages.is_empty() {
        &md.languages[..]
    } else {
        dict_md.map(|m| &m.languages[..]).unwrap_or(&[])
    };
    if !filter.languages.is_empty()
        && !langs.is_empty()
        && !filter.languages.iter().any(|l| langs.contains(l))
    {
        return false;
    }

    let inds = if !md.industries.is_empty() {
        &md.industries[..]
    } else {
        dict_md.map(|m| &m.industries[..]).unwrap_or(&[])
    };
    if !filter.industries.is_empty()
        && !inds.is_empty()
        && !filter.industries.iter().any(|i| inds.contains(i))
    {
        return false;
    }

    let regs = if !md.regions.is_empty() {
        &md.regions[..]
    } else {
        dict_md.map(|m| &m.regions[..]).unwrap_or(&[])
    };
    if !filter.regions.is_empty()
        && !regs.is_empty()
        && !filter.regions.iter().any(|r| regs.contains(r))
    {
        return false;
    }

    let comp = if !md.compliance.is_empty() {
        &md.compliance[..]
    } else {
        dict_md.map(|m| &m.compliance[..]).unwrap_or(&[])
    };
    if !filter.compliance.is_empty()
        && !comp.is_empty()
        && !filter.compliance.iter().any(|c| comp.contains(c))
    {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::EntityKind;

    use super::*;
    use crate::PatternEngine;

    #[test]
    fn untagged_patterns_pass_any_filter() {
        // SSN regex pattern carries no metadata yet (until task 37);
        // a language filter must still let it through.
        let filter = PatternFilter {
            languages: vec!["zu".parse().unwrap()], // Zulu — no pattern lists this
            ..Default::default()
        };
        let engine = PatternEngine::builder()
            .with_filter(filter)
            .build()
            .unwrap();
        let entities =
            engine.scan_entities("SSN: 123-45-6789", &super::super::ScanContext::default());
        assert!(
            entities
                .iter()
                .any(|e| e.entity_kind == EntityKind::GovernmentId),
            "untagged SSN pattern should pass any filter",
        );
    }

    #[test]
    fn filter_drops_tagged_pattern_when_no_overlap() {
        // The `nationalities` pattern is tagged languages=["en"];
        // filter for Zulu should drop it.
        let filter = PatternFilter {
            languages: vec!["zu".parse().unwrap()],
            ..Default::default()
        };
        let engine = PatternEngine::builder()
            .with_filter(filter)
            .build()
            .unwrap();
        let entities =
            engine.scan_entities("She is American.", &super::super::ScanContext::default());
        assert!(
            !entities
                .iter()
                .any(|e| e.entity_kind == EntityKind::Nationality),
            "nationalities (en-tagged) should be filtered out for zu",
        );
    }

    #[test]
    fn filter_keeps_tagged_pattern_when_overlap() {
        let filter = PatternFilter {
            languages: vec!["en".parse().unwrap()],
            ..Default::default()
        };
        let engine = PatternEngine::builder()
            .with_filter(filter)
            .build()
            .unwrap();
        let entities =
            engine.scan_entities("She is American.", &super::super::ScanContext::default());
        assert!(
            entities
                .iter()
                .any(|e| e.entity_kind == EntityKind::Nationality),
            "nationalities should still match for en",
        );
    }

    #[test]
    fn compliance_filter_narrows_regex_patterns() {
        // Only PCI-DSS patterns: credit-card stays, SSN drops.
        let filter = PatternFilter {
            compliance: vec!["pci-dss".to_owned()],
            ..Default::default()
        };
        let engine = PatternEngine::builder()
            .with_filter(filter)
            .build()
            .unwrap();
        let entities = engine.scan_entities(
            "Card 4539 1488 0343 6467 and SSN 123-45-6789.",
            &super::super::ScanContext::default(),
        );
        assert!(
            entities
                .iter()
                .any(|e| e.entity_kind == EntityKind::PaymentCard),
            "credit-card (pci-dss tagged) should match",
        );
        assert!(
            !entities
                .iter()
                .any(|e| e.entity_kind == EntityKind::GovernmentId),
            "SSN (hipaa/glba/ssn-protection tagged) should be filtered out",
        );
    }

    #[test]
    fn region_filter_us_only_drops_iban() {
        // US-only: SSN matches, IBAN does not (iban is eu/global).
        let filter = PatternFilter {
            regions: vec!["us".to_owned()],
            ..Default::default()
        };
        let engine = PatternEngine::builder()
            .with_filter(filter)
            .build()
            .unwrap();
        let entities = engine.scan_entities(
            "SSN 123-45-6789, IBAN GB29NWBK60161331926819.",
            &super::super::ScanContext::default(),
        );
        assert!(
            entities
                .iter()
                .any(|e| e.entity_kind == EntityKind::GovernmentId),
            "SSN should pass us region filter",
        );
        assert!(
            !entities.iter().any(|e| e.entity_kind == EntityKind::Iban),
            "IBAN (eu/global, no us) should be filtered out",
        );
    }

    #[test]
    fn dictionary_allowlist_keeps_only_named_dicts() {
        // `nationalities` is dictionary-backed by dictionary
        // `nationalities`. Restricting the engine to the
        // `nationalities` dictionary should still let it match;
        // other dictionary-backed patterns (e.g. `languages`,
        // `religions`) should be excluded.
        let engine = PatternEngine::builder()
            .with_dictionaries(&["nationalities"])
            .build()
            .unwrap();
        let entities = engine.scan_entities(
            "She is American and speaks English.",
            &super::super::ScanContext::default(),
        );
        assert!(
            entities
                .iter()
                .any(|e| e.entity_kind == EntityKind::Nationality),
            "nationalities pattern (allowed dict) should still match",
        );
        assert!(
            !entities
                .iter()
                .any(|e| e.entity_kind == EntityKind::Language),
            "languages pattern (dict outside allowlist) should be filtered out",
        );
    }

    #[test]
    fn dictionary_allowlist_does_not_affect_regex_patterns() {
        // Restricting dictionaries to an empty-impact name should
        // leave regex patterns (like SSN) entirely untouched.
        let engine = PatternEngine::builder()
            .with_dictionaries(&["nationalities"])
            .build()
            .unwrap();
        let entities = engine.scan_entities(
            "SSN 123-45-6789 here",
            &super::super::ScanContext::default(),
        );
        assert!(
            entities
                .iter()
                .any(|e| e.entity_kind == EntityKind::GovernmentId),
            "SSN regex should still match under a dictionary allowlist",
        );
    }

    #[test]
    fn unconstrained_filter_is_ignored() {
        // Filter with all-empty fields should behave like no filter.
        let engine = PatternEngine::builder()
            .with_filter(PatternFilter::default())
            .build()
            .unwrap();
        let entities = engine.scan_entities(
            "SSN: 123-45-6789 and she is American.",
            &super::super::ScanContext::default(),
        );
        assert!(
            entities
                .iter()
                .any(|e| e.entity_kind == EntityKind::GovernmentId)
        );
        assert!(
            entities
                .iter()
                .any(|e| e.entity_kind == EntityKind::Nationality)
        );
    }
}
