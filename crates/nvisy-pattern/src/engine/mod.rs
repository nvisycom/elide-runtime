//! Pre-compiled pattern matching engine.
//!
//! [`PatternEngine`] compiles all built-in (and optionally user-selected)
//! regex patterns and dictionary automata into a single unit that can
//! scan text in one call.  Use [`PatternEngineBuilder`] for configuration
//! or [`default_engine`] for an out-of-the-box singleton.
//!
//! # Key types
//!
//! - [`PatternEngine`]: the pre-compiled scanning engine.
//! - [`PatternEngineBuilder`]: builder for configuring patterns, thresholds,
//!   and allow/deny lists.
//! - [`PatternMatch`]: a single match produced by scanning.
//! - [`DetectionSource`]: how a match was produced (regex, dictionary, deny list).
//! - [`AllowList`] / [`DenyList`]: exact-match suppression and forced detection.
//! - [`PatternEngineError`]: build-time errors.

mod allow_list;
mod builder;
mod deny_list;
mod error;
mod pattern_match;

pub use allow_list::AllowList;
pub use builder::PatternEngineBuilder;
pub use deny_list::{DenyEntry, DenyList};
pub use error::PatternEngineError;
pub use pattern_match::{DetectionSource, PatternMatch};

use std::sync::LazyLock;

use aho_corasick::AhoCorasick;
use regex::{Regex, RegexSet};

use nvisy_core::data::{EntityCategory, EntityKind};

use crate::patterns::ContextRule;
use crate::validators::ValidatorResolver;

/// Metadata stored alongside each compiled regex.
struct RegexEntry {
    pattern_name: String,
    category: EntityCategory,
    entity_kind: EntityKind,
    confidence: f64,
    validator_name: Option<String>,
    regex: Regex,
    context: Option<ContextRule>,
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
    context: Option<ContextRule>,
}

/// Pre-compiled engine that scans text against all registered patterns.
///
/// Scanning runs in three phases:
///
/// 1. **Regex** — a [`RegexSet`] pre-filter selects candidate patterns,
///    then each matching regex extracts offsets and values.
/// 2. **Dictionary** — Aho-Corasick automata perform literal multi-pattern
///    matching against known-value dictionaries.
/// 3. **Deny list** — known sensitive values not already matched are
///    injected as synthetic matches with confidence `1.0`.
///
/// Allow-list filtering is applied inline during phases 1 and 2.
///
/// Build via [`PatternEngine::builder`] or use [`default_engine`] for
/// the singleton with all built-in patterns.
pub struct PatternEngine {
    regex_set: RegexSet,
    regex_entries: Vec<RegexEntry>,
    dict_entries: Vec<DictEntry>,
    validators: ValidatorResolver,
    confidence_threshold: f64,
    allow_set: AllowList,
    deny_set: DenyList,
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
            EntityKind::Iban => "iban",
            _ => return None,
        };
        let validate = self.validators.resolve(validator_name)?;
        Some(validate(value))
    }

    /// Scan `text` and return all matches above the confidence threshold.
    ///
    /// Matches whose value appears in the allow list are suppressed.
    /// Deny-list values found in the text are injected as synthetic matches
    /// with confidence `1.0` when not already matched.
    #[tracing::instrument(skip(self, text), fields(text_len = text.len(), matches))]
    pub fn scan_text(&self, text: &str) -> Vec<PatternMatch> {
        let mut results = Vec::new();

        self.scan_regex(text, &mut results);
        self.scan_dict(text, &mut results);
        self.scan_deny_list(text, &mut results);

        tracing::Span::current().record("matches", results.len());
        results
    }

    /// Phase 1: regex matches — use `RegexSet` as a pre-filter, then run
    /// each matching regex individually to extract offsets and values.
    fn scan_regex(&self, text: &str, results: &mut Vec<PatternMatch>) {
        let set_matches = self.regex_set.matches(text);
        for idx in set_matches.iter() {
            let entry = &self.regex_entries[idx];

            if entry.confidence < self.confidence_threshold {
                continue;
            }

            for mat in entry.regex.find_iter(text) {
                let value = mat.as_str();

                if self.allow_set.contains(value) {
                    continue;
                }

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
                    context: entry.context.clone(),
                });
            }
        }
    }

    /// Phase 2: dictionary matches via Aho-Corasick automata.
    fn scan_dict(&self, text: &str, results: &mut Vec<PatternMatch>) {
        for entry in &self.dict_entries {
            if entry.confidence < self.confidence_threshold {
                continue;
            }

            for mat in entry.automaton.find_iter(text) {
                let value = &entry.values[mat.pattern().as_usize()];

                if self.allow_set.contains(value.as_str()) {
                    continue;
                }

                results.push(PatternMatch {
                    pattern_name: entry.pattern_name.clone(),
                    category: entry.category.clone(),
                    entity_kind: entry.entity_kind,
                    value: value.clone(),
                    start: mat.start(),
                    end: mat.end(),
                    confidence: entry.confidence,
                    source: DetectionSource::Dictionary,
                    context: entry.context.clone(),
                });
            }
        }
    }

    /// Phase 3: inject deny-list values found in `text` that were not
    /// already matched by regex or dictionary.
    fn scan_deny_list(&self, text: &str, results: &mut Vec<PatternMatch>) {
        for (deny_value, deny_entry) in self.deny_set.iter() {
            if results.iter().any(|r| r.value == deny_value) {
                continue;
            }
            let mut search_start = 0;
            while let Some(pos) = text[search_start..].find(deny_value) {
                let abs_start = search_start + pos;
                let abs_end = abs_start + deny_value.len();
                results.push(PatternMatch {
                    pattern_name: String::new(),
                    category: deny_entry.category.clone(),
                    entity_kind: deny_entry.entity_kind,
                    value: deny_value.to_owned(),
                    start: abs_start,
                    end: abs_end,
                    confidence: 1.0,
                    source: DetectionSource::DenyList,
                    context: None,
                });
                search_start = abs_end;
            }
        }
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
            .with_confidence_threshold(0.99)
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
            .with_patterns(&["email"])
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

    #[test]
    fn allow_list_suppresses_match() {
        let engine = PatternEngine::builder()
            .with_patterns(&["ssn"])
            .with_allow(AllowList::new().with("123-45-6789"))
            .build()
            .unwrap();
        let matches = engine.scan_text("SSN: 123-45-6789");
        assert!(
            !matches.iter().any(|m| m.pattern_name == "ssn"),
            "allow-listed value should be suppressed"
        );
    }

    #[test]
    fn deny_list_injects_match() {
        let deny = DenyList::new()
            .with("secret-value-42", EntityCategory::Pii, EntityKind::PersonName);
        let engine = PatternEngine::builder()
            .with_patterns(&["email"])
            .with_deny(deny)
            .build()
            .unwrap();
        let matches = engine.scan_text("The secret-value-42 should be detected.");
        let deny_match = matches
            .iter()
            .find(|m| m.source == DetectionSource::DenyList)
            .expect("deny list value should be injected");
        assert_eq!(deny_match.value, "secret-value-42");
        assert_eq!(deny_match.confidence, 1.0);
        assert_eq!(deny_match.entity_kind, EntityKind::PersonName);
    }

    #[test]
    fn deny_list_not_injected_when_absent() {
        let deny = DenyList::new()
            .with("not-in-text", EntityCategory::Pii, EntityKind::PersonName);
        let engine = PatternEngine::builder()
            .with_patterns(&["email"])
            .with_deny(deny)
            .build()
            .unwrap();
        let matches = engine.scan_text("Nothing special here.");
        assert!(
            !matches.iter().any(|m| m.source == DetectionSource::DenyList),
            "deny list value not in text should not be injected"
        );
    }

    #[test]
    fn allow_list_from_iterator() {
        let allow: AllowList = ["123-45-6789", "000-00-0000"].into_iter().collect();
        assert_eq!(allow.len(), 2);
        assert!(allow.contains("123-45-6789"));
        assert!(allow.contains("000-00-0000"));
        assert!(!allow.contains("999-99-9999"));
    }

    #[test]
    fn deny_list_from_iterator() {
        let deny: DenyList = [
            ("secret", EntityCategory::Pii, EntityKind::PersonName),
            ("other", EntityCategory::Financial, EntityKind::PaymentCard),
        ]
        .into_iter()
        .collect();
        assert_eq!(deny.len(), 2);
        assert!(deny.contains("secret"));
        let entry = deny.get("other").unwrap();
        assert_eq!(entry.category, EntityCategory::Financial);
    }

    #[test]
    fn context_rule_passthrough() {
        let engine = PatternEngine::builder()
            .with_patterns(&["ssn"])
            .build()
            .unwrap();
        let matches = engine.scan_text("SSN: 123-45-6789");
        let ssn_match = matches.iter().find(|m| m.pattern_name == "ssn").unwrap();
        assert!(
            ssn_match.context.is_some(),
            "SSN pattern should carry context rule through to PatternMatch"
        );
        let ctx = ssn_match.context.as_ref().unwrap();
        assert!(!ctx.keywords.is_empty());
        assert!(ctx.window > 0);
        assert!(ctx.boost > 0.0);
    }
}
