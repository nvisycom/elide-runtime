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
//! - [`RawMatch`]: a single match produced by scanning.
//! - [`AllowList`] / [`DenyList`]: exact-match suppression and forced detection.
//! - [`PatternEngineError`]: build-time errors.

mod allow_list;
mod builder;
mod deny_list;
mod error;
mod pattern_match;

use std::collections::HashSet;
use std::sync::LazyLock;

use aho_corasick::AhoCorasick;
use nvisy_ontology::entity::{EntityCategory, EntityKind, RecognitionMethod};
use regex::{Regex, RegexSet};

pub use self::allow_list::AllowList;
pub use self::builder::PatternEngineBuilder;
pub use self::deny_list::{DenyList, DenyRule};
pub use self::error::PatternEngineError;
pub use self::pattern_match::RawMatch;
use crate::patterns::{ContextRule, DictionaryConfidence};
use crate::validators::ValidatorResolver;

const TARGET: &str = "nvisy_pattern::engine";

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
    confidence: DictionaryConfidence,
    automaton: AhoCorasick,
    /// The terms used to build the automaton, indexed by pattern id.
    values: Vec<String>,
    /// Per-entry column index from the source dictionary (parallel to `values`).
    /// `None` for plain-text dictionaries (all entries are column 0).
    columns: Option<Vec<usize>>,
    context: Option<ContextRule>,
}

impl DictEntry {
    /// Resolve the confidence for the entry at `pattern_index`.
    fn resolve_confidence(&self, pattern_index: usize) -> f64 {
        let col = self
            .columns
            .as_ref()
            .and_then(|cols| cols.get(pattern_index).copied())
            .unwrap_or(0);
        self.confidence.resolve(col)
    }
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

    /// Scan `text` and return all matches above the confidence threshold.
    ///
    /// Matches whose value appears in the allow list are suppressed.
    /// Deny-list values found in the text are injected as synthetic matches
    /// with confidence `1.0` when not already matched.
    #[tracing::instrument(target = TARGET, skip(self, text), fields(text_len = text.len(), matches = tracing::field::Empty))]
    pub fn scan_text(&self, text: &str) -> Vec<RawMatch> {
        let mut results = Vec::new();

        self.scan_regex(text, &mut results);
        self.scan_dict(text, &mut results);
        self.scan_deny_list(text, &mut results);

        tracing::Span::current().record("matches", results.len());
        results
    }

    /// Phase 1: regex matches — use `RegexSet` as a pre-filter, then run
    /// each matching regex individually to extract offsets and values.
    fn scan_regex(&self, text: &str, results: &mut Vec<RawMatch>) {
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

                let mut methods = vec![RecognitionMethod::Regex];

                if let Some(ref vname) = entry.validator_name
                    && let Some(validate) = self.validators.resolve(vname)
                {
                    if !validate(value) {
                        continue;
                    }
                    methods.push(RecognitionMethod::Checksum);
                }

                results.push(RawMatch {
                    pattern_name: Some(entry.pattern_name.clone()),
                    category: entry.category,
                    entity_kind: entry.entity_kind,
                    value: value.to_owned(),
                    start: mat.start(),
                    end: mat.end(),
                    confidence: entry.confidence,
                    recognition_methods: methods,
                    context: entry.context.clone(),
                });
            }
        }
    }

    /// Phase 2: dictionary matches via Aho-Corasick automata.
    fn scan_dict(&self, text: &str, results: &mut Vec<RawMatch>) {
        for entry in &self.dict_entries {
            for mat in entry.automaton.find_iter(text) {
                let pat_idx = mat.pattern().as_usize();
                let value = &entry.values[pat_idx];

                // Resolve per-entry confidence: use column override if available,
                // otherwise fall back to the pattern's base confidence.
                let confidence = entry.resolve_confidence(pat_idx);

                if confidence < self.confidence_threshold {
                    continue;
                }

                if self.allow_set.contains(value.as_str()) {
                    continue;
                }

                results.push(RawMatch {
                    pattern_name: Some(entry.pattern_name.clone()),
                    category: entry.category,
                    entity_kind: entry.entity_kind,
                    value: value.clone(),
                    start: mat.start(),
                    end: mat.end(),
                    confidence,
                    recognition_methods: vec![RecognitionMethod::Dictionary],
                    context: entry.context.clone(),
                });
            }
        }
    }

    /// Phase 3: inject deny-list values found in `text` that were not
    /// already matched by regex or dictionary.
    fn scan_deny_list(&self, text: &str, results: &mut Vec<RawMatch>) {
        let matched_values: HashSet<&str> = results.iter().map(|r| r.value.as_str()).collect();

        let mut deny_matches = Vec::new();
        for (deny_value, deny_rule) in self.deny_set.iter() {
            if matched_values.contains(deny_value) {
                continue;
            }
            let mut search_start = 0;
            while let Some(pos) = text[search_start..].find(deny_value) {
                let abs_start = search_start + pos;
                let abs_end = abs_start + deny_value.len();
                deny_matches.push(RawMatch {
                    pattern_name: None,
                    category: deny_rule.category,
                    entity_kind: deny_rule.entity_kind,
                    value: deny_value.to_owned(),
                    start: abs_start,
                    end: abs_end,
                    confidence: 1.0,
                    recognition_methods: vec![deny_rule.method],
                    context: None,
                });
                search_start = abs_end;
            }
        }
        results.extend(deny_matches);
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
            matches
                .iter()
                .any(|m| m.pattern_name.as_deref() == Some("ssn")),
            "expected SSN match, got: {:?}",
            matches.iter().map(|m| &m.pattern_name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn scan_text_finds_email() {
        let engine = default_engine();
        let matches = engine.scan_text("Contact: alice@example.com");
        assert!(
            matches
                .iter()
                .any(|m| m.pattern_name.as_deref() == Some("email")),
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
            !matches
                .iter()
                .any(|m| m.pattern_name.as_deref() == Some("ssn")),
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
        let ssn_match = matches
            .iter()
            .find(|m| m.pattern_name.as_deref() == Some("ssn"))
            .unwrap();
        assert_eq!(&text[ssn_match.start..ssn_match.end], "123-45-6789");
    }

    #[test]
    fn dictionary_matches_are_found() {
        let engine = default_engine();
        let matches = engine.scan_text("She is American and speaks English.");
        assert!(
            matches.iter().any(|m| m
                .recognition_methods
                .contains(&RecognitionMethod::Dictionary)),
            "expected dictionary match, got: {:?}",
            matches
                .iter()
                .map(|m| (&m.pattern_name, &m.recognition_methods))
                .collect::<Vec<_>>()
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
            !matches
                .iter()
                .any(|m| m.pattern_name.as_deref() == Some("ssn")),
            "allow-listed value should be suppressed"
        );
    }

    #[test]
    fn deny_list_injects_match() {
        let deny = DenyList::new().with(
            "secret-value-42",
            EntityCategory::PersonalIdentity,
            EntityKind::PersonName,
        );
        let engine = PatternEngine::builder()
            .with_patterns(&["email"])
            .with_deny(deny)
            .build()
            .unwrap();
        let matches = engine.scan_text("The secret-value-42 should be detected.");
        let deny_match = matches
            .iter()
            .find(|m| m.pattern_name.is_none())
            .expect("deny list value should be injected");
        assert_eq!(deny_match.value, "secret-value-42");
        assert_eq!(deny_match.confidence, 1.0);
        assert_eq!(deny_match.entity_kind, EntityKind::PersonName);
        assert_eq!(
            deny_match.recognition_methods,
            vec![RecognitionMethod::Dictionary]
        );
    }

    #[test]
    fn deny_list_not_injected_when_absent() {
        let deny = DenyList::new().with(
            "not-in-text",
            EntityCategory::PersonalIdentity,
            EntityKind::PersonName,
        );
        let engine = PatternEngine::builder()
            .with_patterns(&["email"])
            .with_deny(deny)
            .build()
            .unwrap();
        let matches = engine.scan_text("Nothing special here.");
        assert!(
            !matches.iter().any(|m| m.pattern_name.is_none()),
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
            (
                "secret",
                EntityCategory::PersonalIdentity,
                EntityKind::PersonName,
            ),
            ("other", EntityCategory::Financial, EntityKind::PaymentCard),
        ]
        .into_iter()
        .collect();
        assert_eq!(deny.len(), 2);
        assert!(deny.contains("secret"));
        let rule = deny.get("other").unwrap();
        assert_eq!(rule.category, EntityCategory::Financial);
    }

    #[test]
    fn column_confidence_applies_to_csv_dictionaries() {
        let engine = default_engine();
        // "US Dollar" is column 0 (full name), "USD" is column 1 (code).
        let matches = engine.scan_text("I paid in US Dollar and also in USD.");
        let full_name = matches.iter().find(|m| m.value == "US Dollar");
        let code = matches.iter().find(|m| m.value == "USD");
        assert!(full_name.is_some(), "should match 'US Dollar'");
        assert!(code.is_some(), "should match 'USD'");
        let full_conf = full_name.unwrap().confidence;
        let code_conf = code.unwrap().confidence;
        assert!(
            full_conf > code_conf,
            "full name confidence ({full_conf}) should exceed code confidence ({code_conf})"
        );
    }

    #[test]
    fn context_rule_passthrough() {
        let engine = PatternEngine::builder()
            .with_patterns(&["ssn"])
            .build()
            .unwrap();
        let matches = engine.scan_text("SSN: 123-45-6789");
        let ssn_match = matches
            .iter()
            .find(|m| m.pattern_name.as_deref() == Some("ssn"))
            .unwrap();
        assert!(
            ssn_match.context.is_some(),
            "SSN pattern should carry context rule through to RawMatch"
        );
        let ctx = ssn_match.context.as_ref().unwrap();
        assert!(!ctx.keywords.is_empty());
        assert!(ctx.window > 0);
        assert!(ctx.boost > 0.0);
    }

    #[test]
    fn into_entity_builds_entity_without_location() {
        let raw = RawMatch {
            pattern_name: Some("ssn".into()),
            category: EntityCategory::PersonalIdentity,
            entity_kind: EntityKind::GovernmentId,
            value: "123-45-6789".into(),
            start: 5,
            end: 16,
            confidence: 0.9,
            recognition_methods: vec![RecognitionMethod::Regex, RecognitionMethod::Checksum],
            context: None,
        };
        let entity = raw.into_entity();
        assert_eq!(entity.value, "123-45-6789");
        assert_eq!(entity.entity_kind, EntityKind::GovernmentId);
        assert_eq!(
            entity.recognition_methods,
            vec![RecognitionMethod::Regex, RecognitionMethod::Checksum]
        );
        assert!((entity.confidence - 0.9).abs() < f64::EPSILON);
        assert!(entity.location.is_none());
    }
}
