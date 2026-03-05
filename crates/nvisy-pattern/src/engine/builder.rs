//! [`PatternEngineBuilder`] — configures and compiles a [`PatternEngine`].

use regex::{Regex, RegexSet};

use super::allow_list::AllowList;
use super::deny_list::DenyList;
use super::error::PatternEngineError;
use super::{DictEntry, PatternEngine, RegexEntry};
use crate::dictionaries;
use crate::patterns::{self, MatchSource, Pattern};
use crate::validators::ValidatorResolver;

/// Builder for [`PatternEngine`].
///
/// By default all built-in patterns are included. Use
/// [`patterns`](Self::patterns) to restrict to a subset.
#[derive(Default)]
pub struct PatternEngineBuilder {
    pattern_names: Option<Vec<String>>,
    confidence_threshold: f64,
    allow_list: AllowList,
    deny_list: DenyList,
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
    /// [`scan_text`](PatternEngine::scan_text).  Defaults to `0.0`.
    pub fn with_confidence_threshold(mut self, threshold: f64) -> Self {
        self.confidence_threshold = threshold;
        self
    }

    /// Set the allow list.
    ///
    /// Matches whose exact value appears in the allow list are suppressed
    /// (dropped) during [`scan_text`](PatternEngine::scan_text).
    pub fn with_allow(mut self, list: AllowList) -> Self {
        self.allow_list = list;
        self
    }

    /// Set the deny list.
    ///
    /// If a deny-list value is found in the scanned text but was not matched
    /// by any regex or dictionary pattern, it is injected as a synthetic match
    /// with confidence `1.0`.
    pub fn with_deny(mut self, list: DenyList) -> Self {
        self.deny_list = list;
        self
    }

    /// Compile all selected patterns and build the engine.
    ///
    /// # Errors
    ///
    /// Returns [`PatternEngineError`] if a regex fails to compile, a
    /// referenced dictionary is missing, or the Aho-Corasick automaton
    /// cannot be built.
    #[tracing::instrument(name = "PatternEngine::build", skip(self))]
    pub fn build(self) -> Result<PatternEngine, PatternEngineError> {
        let pat_reg = patterns::builtin_registry();
        let dict_reg = dictionaries::builtin_registry();

        let active: Vec<&dyn Pattern> = match &self.pattern_names {
            Some(names) => names.iter().filter_map(|n| pat_reg.get(n)).collect(),
            None => pat_reg.values(),
        };

        let mut regex_entries = Vec::new();
        let mut regex_strings = Vec::new();
        let mut dict_entries = Vec::new();

        for p in &active {
            match p.match_source() {
                MatchSource::Regex(rp) => {
                    let compiled =
                        Regex::new(&rp.regex).map_err(|e| PatternEngineError::RegexCompile {
                            name: p.name().to_owned(),
                            source: e,
                        })?;
                    regex_strings.push(rp.regex.clone());
                    regex_entries.push(RegexEntry {
                        pattern_name: p.name().to_owned(),
                        category: p.category().clone(),
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
                    let values: Vec<String> = dict.entries().to_vec();
                    if values.is_empty() {
                        continue;
                    }
                    let columns = dict.columns().map(|c| c.to_vec());
                    let automaton = aho_corasick::AhoCorasickBuilder::new()
                        .ascii_case_insensitive(!dp.case_sensitive)
                        .build(&values)
                        .map_err(|e| PatternEngineError::AhoCorasickBuild {
                            name: p.name().to_owned(),
                            source: e,
                        })?;
                    dict_entries.push(DictEntry {
                        pattern_name: p.name().to_owned(),
                        category: p.category().clone(),
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
            allow_set: self.allow_list,
            deny_set: self.deny_list,
        })
    }
}
