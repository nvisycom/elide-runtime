//! [`PatternEngineBuilder`]: configures and compiles a [`PatternEngine`].

use nvisy_ontology::workflow::PatternFilter;
use regex::{Regex, RegexSet};

use super::PatternEngine;
use super::error::PatternEngineError;
use super::scan::entries::{DictEntry, RegexEntry};
use crate::dictionaries;
use crate::patterns::{MatchSource, Pattern};
use crate::validators::ValidatorResolver;

const TARGET: &str = "nvisy_pattern::engine";

/// Builder for [`PatternEngine`].
///
/// By default all built-in patterns are included. Use
/// [`with_patterns`] to restrict to a subset by name, or
/// [`with_filter`] to narrow by metadata tags.
///
/// [`with_patterns`]: Self::with_patterns
/// [`with_filter`]: Self::with_filter
#[derive(Default)]
pub struct PatternEngineBuilder {
    pattern_names: Option<Vec<String>>,
    confidence_threshold: f64,
    filter: Option<PatternFilter>,
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
    /// is included only when its [`PatternMetadata`] satisfies every
    /// non-empty constraint in `filter`. A pattern with no tags on a
    /// particular axis is treated as **universal** on that axis (it
    /// passes any filter for that field).
    ///
    /// [`PatternMetadata`]: crate::patterns::PatternMetadata
    pub fn with_filter(mut self, filter: PatternFilter) -> Self {
        if !filter.is_unconstrained() {
            self.filter = Some(filter);
        }
        self
    }

    /// Compile all selected patterns and build the engine.
    ///
    /// # Errors
    ///
    /// Returns [`nvisy_core::Error`] if a regex fails to compile, a
    /// referenced dictionary is missing, or the Aho-Corasick automaton
    /// cannot be built.
    #[tracing::instrument(target = TARGET, name = "PatternEngine::build", skip(self))]
    pub fn build(self) -> nvisy_core::Result<PatternEngine> {
        let pat_reg = crate::patterns::builtin_registry();
        let dict_reg = dictionaries::builtin_registry();

        let active: Vec<&dyn Pattern> = match &self.pattern_names {
            Some(names) => names.iter().filter_map(|n| pat_reg.get(n)).collect(),
            None => pat_reg.iter().collect(),
        };

        let mut regex_entries = Vec::new();
        let mut regex_strings = Vec::new();
        let mut dict_entries = Vec::new();

        for p in &active {
            if let Some(ref filter) = self.filter
                && !pattern_matches_filter(*p, filter)
            {
                tracing::trace!(
                    target: TARGET,
                    pattern = p.name(),
                    "skipped by pattern filter",
                );
                continue;
            }

            match p.match_source() {
                MatchSource::Regex(rp) => {
                    let effective = rp.effective_regex();
                    let compiled =
                        Regex::new(&effective).map_err(|e| PatternEngineError::RegexCompile {
                            name: p.name().to_owned(),
                            source: e,
                        })?;
                    regex_strings.push(effective);
                    regex_entries.push(RegexEntry {
                        pattern_name: p.name().to_owned(),
                        category: p.category(),
                        entity_kind: p.entity_kind(),
                        confidence: rp.confidence,
                        validator_name: rp.validator.clone(),
                        regex: compiled,
                        context: p.context().cloned(),
                    });
                }
                MatchSource::Dictionary(dp) => {
                    let dict = dict_reg.get(&dp.name).ok_or_else(|| {
                        PatternEngineError::UnknownDictionary {
                            name: p.name().to_owned(),
                            dictionary: dp.name.clone(),
                        }
                    })?;
                    let terms: Vec<_> = dict.terms().to_vec();
                    if terms.is_empty() {
                        continue;
                    }
                    let automaton = aho_corasick::AhoCorasickBuilder::new()
                        .ascii_case_insensitive(!dp.case_sensitive)
                        .build(terms.iter().map(|t| t.value.as_str()))
                        .map_err(|e| PatternEngineError::AhoCorasickBuild {
                            name: p.name().to_owned(),
                            source: e,
                        })?;
                    dict_entries.push(DictEntry {
                        pattern_name: p.name().to_owned(),
                        category: p.category(),
                        entity_kind: p.entity_kind(),
                        confidence: dp.confidence.clone(),
                        automaton,
                        terms,
                        context: p.context().cloned(),
                    });
                }
            }
        }

        let regex_set = RegexSet::new(&regex_strings).map_err(PatternEngineError::RegexSetBuild)?;

        let validators = ValidatorResolver::builtins();

        tracing::debug!(
            target: TARGET,
            regex_count = regex_entries.len(),
            dict_count = dict_entries.len(),
            "PatternEngine built",
        );

        Ok(PatternEngine {
            regex_set,
            regex_entries,
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
/// - filter non-empty, pattern field empty → pattern is universal on
///   this axis, always passes
/// - both non-empty → tags must overlap (OR within field)
///
/// Across fields the test is AND.
fn pattern_matches_filter(p: &dyn Pattern, filter: &PatternFilter) -> bool {
    let md = p.metadata();

    if !filter.languages.is_empty()
        && !md.languages.is_empty()
        && !filter.languages.iter().any(|l| md.languages.contains(l))
    {
        return false;
    }
    if !filter.industries.is_empty()
        && !md.industries.is_empty()
        && !filter.industries.iter().any(|i| md.industries.contains(i))
    {
        return false;
    }
    if !filter.regions.is_empty()
        && !md.regions.is_empty()
        && !filter.regions.iter().any(|r| md.regions.contains(r))
    {
        return false;
    }
    if !filter.compliance.is_empty()
        && !md.compliance.is_empty()
        && !filter.compliance.iter().any(|c| md.compliance.contains(c))
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
