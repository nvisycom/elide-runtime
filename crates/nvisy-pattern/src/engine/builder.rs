//! [`PatternEngineBuilder`]: configures and compiles a [`PatternEngine`].

use regex::{Regex, RegexSet};

use super::error::PatternEngineError;
use super::{DictEntry, PatternEngine, RegexEntry, TARGET};
use crate::dictionaries;
use crate::patterns::{MatchSource, Pattern};
use crate::validators::ValidatorResolver;

/// Builder for [`PatternEngine`].
///
/// By default all built-in patterns are included. Use
/// [`with_patterns`] to restrict to a subset.
///
/// [`with_patterns`]: Self::with_patterns
#[derive(Default)]
pub struct PatternEngineBuilder {
    pattern_names: Option<Vec<String>>,
    confidence_threshold: f64,
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
                    let terms = dict.terms();
                    if terms.is_empty() {
                        continue;
                    }
                    let values: Vec<String> = terms.iter().map(|t| t.value.clone()).collect();
                    let columns: Vec<Option<u32>> = terms.iter().map(|t| t.column).collect();
                    let automaton = aho_corasick::AhoCorasickBuilder::new()
                        .ascii_case_insensitive(!dp.case_sensitive)
                        .build(&values)
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
                        values,
                        columns,
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
