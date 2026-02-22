//! Pre-compiled pattern matching engine.
//!
//! [`PatternEngine`] compiles all built-in (and optionally user-selected)
//! regex patterns and dictionary automata into a single unit that can
//! scan text in one pass.  Use [`PatternEngineBuilder`] for configuration
//! or [`default_engine`] for an out-of-the-box singleton.

mod builder;
mod types;

pub use builder::PatternEngineBuilder;
pub use types::{DetectionSource, PatternEngineError, PatternMatch};

use std::sync::LazyLock;

use aho_corasick::AhoCorasick;
use regex::{Regex, RegexSet};

use nvisy_core::data::{EntityCategory, EntityKind};

use crate::validators::ValidatorResolver;

/// Metadata stored alongside each compiled regex.
struct RegexEntry {
    pattern_name: String,
    category: EntityCategory,
    entity_kind: EntityKind,
    confidence: f64,
    validator_name: Option<String>,
    regex: Regex,
}

/// Metadata stored alongside each compiled Aho-Corasick automaton.
struct DictEntry {
    pattern_name: String,
    category: EntityCategory,
    entity_kind: EntityKind,
    confidence: f64,
    automaton: AhoCorasick,
    /// The terms used to build the automaton, indexed by pattern id.
    values: Vec<String>,
}

/// Pre-compiled engine that scans text against all registered patterns.
///
/// Build via [`PatternEngine::builder`] or use [`default_engine`] for
/// the singleton with all built-in patterns.
pub struct PatternEngine {
    regex_set: RegexSet,
    regex_entries: Vec<RegexEntry>,
    dict_entries: Vec<DictEntry>,
    validators: ValidatorResolver,
    confidence_threshold: f64,
}

impl std::fmt::Debug for PatternEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PatternEngine")
            .field("regex_patterns", &self.regex_entries.len())
            .field("dict_patterns", &self.dict_entries.len())
            .field("confidence_threshold", &self.confidence_threshold)
            .finish()
    }
}

impl PatternEngine {
    /// Create a new [`PatternEngineBuilder`].
    pub fn builder() -> PatternEngineBuilder {
        PatternEngineBuilder::default()
    }

    /// Validate a value using the checksum associated with the entity kind.
    ///
    /// Returns `Some(true)` if the value passes, `Some(false)` if it fails,
    /// or `None` if no checksum validator is registered for that entity kind.
    pub fn validate_checksum(&self, entity_kind: EntityKind, value: &str) -> Option<bool> {
        let validator_name = match entity_kind {
            EntityKind::PaymentCard => "luhn",
            EntityKind::GovernmentId => "ssn",
            _ => return None,
        };
        let validate = self.validators.resolve(validator_name)?;
        Some(validate(value))
    }

    /// Scan `text` and return all matches above the confidence threshold.
    #[tracing::instrument(skip(self, text), fields(text_len = text.len(), matches))]
    pub fn scan_text(&self, text: &str) -> Vec<PatternMatch> {
        let mut results = Vec::new();

        // Phase 1: regex matches — use RegexSet as a pre-filter.
        let set_matches = self.regex_set.matches(text);
        for idx in set_matches.iter() {
            let entry = &self.regex_entries[idx];

            if entry.confidence < self.confidence_threshold {
                continue;
            }

            for mat in entry.regex.find_iter(text) {
                let value = mat.as_str();

                if let Some(ref vname) = entry.validator_name {
                    if let Some(validate) = self.validators.resolve(vname) {
                        if !validate(value) {
                            continue;
                        }
                    }
                }

                results.push(PatternMatch {
                    pattern_name: entry.pattern_name.clone(),
                    category: entry.category.clone(),
                    entity_kind: entry.entity_kind,
                    value: value.to_owned(),
                    start: mat.start(),
                    end: mat.end(),
                    confidence: entry.confidence,
                    source: DetectionSource::Regex,
                });
            }
        }

        // Phase 2: dictionary matches.
        for entry in &self.dict_entries {
            if entry.confidence < self.confidence_threshold {
                continue;
            }

            for mat in entry.automaton.find_iter(text) {
                let value = &entry.values[mat.pattern().as_usize()];

                results.push(PatternMatch {
                    pattern_name: entry.pattern_name.clone(),
                    category: entry.category.clone(),
                    entity_kind: entry.entity_kind,
                    value: value.clone(),
                    start: mat.start(),
                    end: mat.end(),
                    confidence: entry.confidence,
                    source: DetectionSource::Dictionary,
                });
            }
        }

        tracing::Span::current().record("matches", results.len());
        results
    }
}

static DEFAULT_ENGINE: LazyLock<PatternEngine> = LazyLock::new(|| {
    PatternEngine::builder()
        .build()
        .expect("built-in patterns must compile")
});

/// Return a reference to the lazily-initialised default [`PatternEngine`]
/// containing all built-in patterns.
pub fn default_engine() -> &'static PatternEngine {
    &DEFAULT_ENGINE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_engine_builds() {
        let engine = default_engine();
        assert!(!engine.regex_entries.is_empty());
    }

    #[test]
    fn scan_text_finds_ssn() {
        let engine = default_engine();
        let matches = engine.scan_text("My SSN is 123-45-6789.");
        assert!(
            matches.iter().any(|m| m.pattern_name == "ssn"),
            "expected SSN match, got: {:?}",
            matches.iter().map(|m| &m.pattern_name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn scan_text_finds_email() {
        let engine = default_engine();
        let matches = engine.scan_text("Contact: alice@example.com");
        assert!(
            matches.iter().any(|m| m.pattern_name == "email"),
            "expected email match, got: {:?}",
            matches.iter().map(|m| &m.pattern_name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn confidence_threshold_filters() {
        let engine = PatternEngine::builder()
            .confidence_threshold(0.99)
            .build()
            .unwrap();
        let matches = engine.scan_text("My SSN is 123-45-6789.");
        assert!(
            !matches.iter().any(|m| m.pattern_name == "ssn"),
            "SSN should be filtered by 0.99 threshold"
        );
    }

    #[test]
    fn builder_pattern_filter() {
        let engine = PatternEngine::builder()
            .patterns(&["email"])
            .build()
            .unwrap();
        assert_eq!(engine.regex_entries.len(), 1);
        assert_eq!(engine.regex_entries[0].pattern_name, "email");
    }

    #[test]
    fn scan_text_returns_correct_offsets() {
        let engine = default_engine();
        let text = "SSN: 123-45-6789";
        let matches = engine.scan_text(text);
        let ssn_match = matches.iter().find(|m| m.pattern_name == "ssn").unwrap();
        assert_eq!(&text[ssn_match.start..ssn_match.end], "123-45-6789");
    }

    #[test]
    fn dictionary_matches_are_found() {
        let engine = default_engine();
        let matches = engine.scan_text("She is American and speaks English.");
        assert!(
            matches.iter().any(|m| m.source == DetectionSource::Dictionary),
            "expected dictionary match, got: {:?}",
            matches.iter().map(|m| (&m.pattern_name, &m.source)).collect::<Vec<_>>()
        );
    }
}
