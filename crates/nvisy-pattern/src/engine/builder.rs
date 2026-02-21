//! [`PatternEngineBuilder`] — configures and compiles a [`PatternEngine`].

use regex::{Regex, RegexSet};

use crate::dictionaries;
use crate::patterns::{self, MatchSource, Pattern};
use crate::validators::ValidatorResolver;

use super::types::PatternEngineError;
use super::{DictEntry, PatternEngine, RegexEntry};

/// Builder for [`PatternEngine`].
///
/// By default all built-in patterns are included. Use
/// [`patterns`](Self::patterns) to restrict to a subset.
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
    pub fn patterns(mut self, names: &[impl AsRef<str>]) -> Self {
        if !names.is_empty() {
            self.pattern_names = Some(names.iter().map(|n| n.as_ref().to_owned()).collect());
        }
        self
    }

    /// Set the minimum confidence score for matches.
    ///
    /// Matches with confidence below this value are discarded during
    /// [`scan_text`](PatternEngine::scan_text).  Defaults to `0.0`.
    pub fn confidence_threshold(mut self, threshold: f64) -> Self {
        self.confidence_threshold = threshold;
        self
    }

    /// Compile all selected patterns and build the engine.
    #[tracing::instrument(name = "PatternEngine::build", skip(self))]
    pub fn build(self) -> Result<PatternEngine, PatternEngineError> {
        let pat_reg = patterns::builtin_registry();
        let dict_reg = dictionaries::builtin_registry();

        let active: Vec<&dyn Pattern> = match &self.pattern_names {
            Some(names) => names
                .iter()
                .filter_map(|n| pat_reg.get(n))
                .collect(),
            None => pat_reg.values(),
        };

        let mut regex_entries = Vec::new();
        let mut regex_strings = Vec::new();
        let mut dict_entries = Vec::new();

        for p in &active {
            match p.match_source() {
                MatchSource::Regex(re) => {
                    let compiled = Regex::new(re).map_err(|e| PatternEngineError::RegexCompile {
                        name: p.name().to_owned(),
                        source: e,
                    })?;
                    regex_strings.push(re.clone());
                    regex_entries.push(RegexEntry {
                        pattern_name: p.name().to_owned(),
                        category: p.category().clone(),
                        entity_kind: p.entity_kind(),
                        confidence: p.confidence(),
                        validator_name: p.validator_name().map(|s| s.to_owned()),
                        regex: compiled,
                    });
                }
                MatchSource::Dictionary(dict_name) => {
                    let dict = dict_reg.get(dict_name).ok_or_else(|| {
                        PatternEngineError::UnknownDictionary {
                            name: p.name().to_owned(),
                            dictionary: dict_name.clone(),
                        }
                    })?;
                    let values: Vec<String> = dict.entries().to_vec();
                    if values.is_empty() {
                        continue;
                    }
                    let automaton = aho_corasick::AhoCorasickBuilder::new()
                        .ascii_case_insensitive(!p.case_sensitive())
                        .build(&values)
                        .map_err(|e| PatternEngineError::AhoCorasickBuild {
                            name: p.name().to_owned(),
                            source: e,
                        })?;
                    dict_entries.push(DictEntry {
                        pattern_name: p.name().to_owned(),
                        category: p.category().clone(),
                        entity_kind: p.entity_kind(),
                        confidence: p.confidence(),
                        automaton,
                        values,
                    });
                }
            }
        }

        let regex_set =
            RegexSet::new(&regex_strings).map_err(PatternEngineError::RegexSetBuild)?;

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
        })
    }
}
