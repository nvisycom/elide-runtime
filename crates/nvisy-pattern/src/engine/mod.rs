//! Pre-compiled pattern matching engine.
//!
//! [`PatternEngine`] compiles all built-in (and optionally user-selected)
//! regex patterns and dictionary automata into a single unit that can
//! scan text in one call. Use [`PatternEngine::builder`] for configuration
//! or [`PatternEngine::instance`] for an out-of-the-box singleton.
//!
//! # Key types
//!
//! - [`PatternEngine`]: pre-compiled scanning engine.
//! - [`ScanContext`]: per-scan allow/deny list configuration.
//! - [`RawMatch`]: single match produced by scanning.
//! - [`AllowList`] / [`DenyList`]: exact-match suppression and forced detection.
//! - [`PatternEngineBuilder`]: builder for configuring patterns and thresholds.

mod allow_list;
mod builder;
mod deny_list;
mod error;
mod pattern_match;
mod scan_context;

use std::collections::HashSet;
use std::sync::LazyLock;

use aho_corasick::AhoCorasick;
use nvisy_ontology::entity::{Entity, EntityCategory, EntityKind, RecognitionMethod};
use regex::{Regex, RegexSet};

pub use self::allow_list::AllowList;
pub use self::builder::PatternEngineBuilder;
pub use self::deny_list::{DenyList, DenyRule};
pub(crate) use self::pattern_match::RawMatch;
pub use self::scan_context::ScanContext;
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
    /// `None` entries indicate plain-text origin (logically column 0).
    columns: Vec<Option<u32>>,
    context: Option<ContextRule>,
}

impl DictEntry {
    /// Resolve the confidence for the entry at `pattern_index`.
    fn resolve_confidence(&self, pattern_index: usize) -> f64 {
        let col = self
            .columns
            .get(pattern_index)
            .copied()
            .flatten()
            .unwrap_or(0) as usize;
        self.confidence.resolve(col)
    }
}

/// Pre-compiled engine that scans text against all registered patterns.
///
/// Scanning runs in three phases:
///
/// 1. **Regex**: a [`RegexSet`] pre-filter selects candidate patterns,
///    then each matching regex extracts offsets and values.
/// 2. **Dictionary**: Aho-Corasick automata perform literal multi-pattern
///    matching against known-value dictionaries.
/// 3. **Deny list**: known sensitive values not already matched are
///    injected as synthetic matches with confidence `1.0`.
///
/// Allow-list filtering is applied inline during phases 1 and 2.
///
/// Build via [`PatternEngine::builder`] or use [`PatternEngine::instance`]
/// for the singleton with all built-in patterns.
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

impl std::fmt::Display for PatternEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PatternEngine({} regex, {} dict, threshold {:.2})",
            self.regex_entries.len(),
            self.dict_entries.len(),
            self.confidence_threshold,
        )
    }
}

impl PatternEngine {
    /// Return a reference to the lazily-initialised default engine
    /// containing all built-in patterns.
    pub fn instance() -> &'static Self {
        &DEFAULT_ENGINE
    }

    /// Create a new [`PatternEngineBuilder`].
    pub fn builder() -> PatternEngineBuilder {
        PatternEngineBuilder::default()
    }

    /// Scan `text` and return all matches above the confidence threshold.
    ///
    /// Matches whose value appears in the allow list are suppressed.
    /// Deny-list values found in the text are injected as synthetic matches
    /// with confidence `1.0` when not already matched.
    /// Scan `text` and return detected entities with [`TextLocation`]s.
    ///
    /// Each entity carries a [`TextLocation`] with `start_offset` and
    /// `end_offset` set from the match.
    #[tracing::instrument(target = TARGET, skip(self, text, ctx), fields(text_len = text.len(), entities = tracing::field::Empty))]
    pub fn scan_entities(&self, text: &str, ctx: &ScanContext) -> Vec<Entity> {
        let mut raw = self.scan_raw(text, ctx);

        // Apply context-based confidence adjustment before threshold
        // filtering so that a match just below threshold can be
        // promoted by keyword co-occurrence.
        for m in &mut raw {
            m.apply_context_adjustment(text);
        }

        // Deduplicate overlapping matches of the same entity kind:
        // sort by (kind, start, descending confidence) then keep only
        // the highest-confidence match per overlapping span.
        raw.sort_by(|a, b| {
            a.start.cmp(&b.start).then(
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        let mut deduped: Vec<RawMatch> = Vec::with_capacity(raw.len());
        for m in raw {
            let dominated = deduped.iter().any(|existing| {
                existing.entity_kind == m.entity_kind
                    && existing.start <= m.start
                    && existing.end >= m.end
                    && existing.confidence >= m.confidence
            });
            if !dominated {
                deduped.push(m);
            }
        }

        let threshold = self.confidence_threshold;
        let entities: Vec<Entity> = deduped
            .into_iter()
            .filter(|m| m.confidence >= threshold)
            .map(|m| {
                tracing::trace!(
                    target: TARGET,
                    pattern = m.pattern_name.as_deref().unwrap_or("deny_list"),
                    kind = %m.entity_kind,
                    confidence = m.confidence,
                    start = m.start,
                    end = m.end,
                    "matched entity",
                );

                m.into_entity()
            })
            .collect();

        tracing::Span::current().record("entities", entities.len());
        entities
    }

    /// Internal: scan and return raw matches (used by [`scan_entities`]
    /// and tests).
    fn scan_raw(&self, text: &str, ctx: &ScanContext) -> Vec<RawMatch> {
        let mut results = Vec::new();

        self.scan_regex(text, &ctx.allow, &mut results);
        self.scan_dict(text, &ctx.allow, &mut results);
        self.scan_deny_list(text, &ctx.deny, &mut results);

        results
    }

    /// Phase 1: regex matches. Uses `RegexSet` as a pre-filter, then runs
    /// each matching regex individually to extract offsets and values.
    fn scan_regex(&self, text: &str, allow: &AllowList, results: &mut Vec<RawMatch>) {
        let set_matches = self.regex_set.matches(text);
        for idx in set_matches.iter() {
            let entry = &self.regex_entries[idx];

            for mat in entry.regex.find_iter(text) {
                let value = mat.as_str();

                if allow.contains(value) {
                    continue;
                }

                if let Some(ref vname) = entry.validator_name
                    && let Some(validate) = self.validators.resolve(vname)
                    && !validate(value)
                {
                    continue;
                }

                let method = if let Some(ref vname) = entry.validator_name {
                    RecognitionMethod::regex_validated(&entry.pattern_name, vname)
                } else {
                    RecognitionMethod::regex(&entry.pattern_name)
                };

                results.push(RawMatch {
                    pattern_name: Some(entry.pattern_name.clone()),
                    category: entry.category,
                    entity_kind: entry.entity_kind,
                    value: value.to_owned(),
                    start: mat.start(),
                    end: mat.end(),
                    confidence: entry.confidence,
                    recognition_methods: vec![method],
                    context: entry.context.clone(),
                });
            }
        }
    }

    /// Phase 2: dictionary matches via Aho-Corasick automata.
    fn scan_dict(&self, text: &str, allow: &AllowList, results: &mut Vec<RawMatch>) {
        for entry in &self.dict_entries {
            for mat in entry.automaton.find_iter(text) {
                let pat_idx = mat.pattern().as_usize();
                let value = &entry.values[pat_idx];

                let confidence = entry.resolve_confidence(pat_idx);

                if allow.contains(value.as_str()) {
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
                    recognition_methods: vec![RecognitionMethod::dictionary(&entry.pattern_name)],
                    context: entry.context.clone(),
                });
            }
        }
    }

    /// Phase 3: inject deny-list values found in `text` not already
    /// matched by regex or dictionary.
    fn scan_deny_list(&self, text: &str, deny: &DenyList, results: &mut Vec<RawMatch>) {
        let matched_values: HashSet<&str> = results.iter().map(|r| r.value.as_str()).collect();

        let mut deny_matches = Vec::new();
        for (deny_value, deny_rule) in deny.iter() {
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
                    recognition_methods: vec![deny_rule.method.clone()],
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

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::ModelKind;

    use super::*;

    fn empty_ctx() -> ScanContext {
        ScanContext::default()
    }

    #[test]
    fn default_engine_builds() {
        let engine = PatternEngine::instance();
        assert!(!engine.regex_entries.is_empty());
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
    fn scan_raw_returns_correct_offsets() {
        let engine = PatternEngine::instance();
        let text = "SSN: 123-45-6789";
        let matches = engine.scan_raw(text, &empty_ctx());
        let ssn_match = matches
            .iter()
            .find(|m| m.pattern_name.as_deref() == Some("ssn"))
            .unwrap();
        assert_eq!(&text[ssn_match.start..ssn_match.end], "123-45-6789");
    }

    #[test]
    fn allow_list_suppresses_match() {
        let engine = PatternEngine::builder()
            .with_patterns(&["ssn"])
            .build()
            .unwrap();
        let ctx = ScanContext::new().with_allow(AllowList::new().with("123-45-6789"));
        let matches = engine.scan_raw("SSN: 123-45-6789", &ctx);
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
            DenyRule {
                category: EntityCategory::PersonalIdentity,
                entity_kind: EntityKind::PersonName,
                method: RecognitionMethod::ner("test", ModelKind::SelfHosted),
            },
        );
        let engine = PatternEngine::builder()
            .with_patterns(&["email"])
            .build()
            .unwrap();
        let ctx = ScanContext::new().with_deny(deny);
        let matches = engine.scan_raw("The secret-value-42 should be detected.", &ctx);
        let deny_match = matches
            .iter()
            .find(|m| m.pattern_name.is_none())
            .expect("deny list value should be injected");
        assert_eq!(deny_match.value, "secret-value-42");
        assert_eq!(deny_match.confidence, 1.0);
        assert_eq!(deny_match.entity_kind, EntityKind::PersonName);
        assert_eq!(
            deny_match.recognition_methods,
            vec![RecognitionMethod::ner("test", ModelKind::SelfHosted)]
        );
    }

    #[test]
    fn deny_list_not_injected_when_absent() {
        let deny = DenyList::new().with(
            "not-in-text",
            DenyRule {
                category: EntityCategory::PersonalIdentity,
                entity_kind: EntityKind::PersonName,
                method: RecognitionMethod::annotation(Some("test".into())),
            },
        );
        let engine = PatternEngine::builder()
            .with_patterns(&["email"])
            .build()
            .unwrap();
        let ctx = ScanContext::new().with_deny(deny);
        let matches = engine.scan_raw("Nothing special here.", &ctx);
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
    fn deny_list_insert_and_lookup() {
        let mut deny = DenyList::new();
        deny.insert(
            "secret",
            DenyRule {
                category: EntityCategory::PersonalIdentity,
                entity_kind: EntityKind::PersonName,
                method: RecognitionMethod::ner("test", ModelKind::SelfHosted),
            },
        );
        deny.insert(
            "other",
            DenyRule {
                category: EntityCategory::Financial,
                entity_kind: EntityKind::PaymentCard,
                method: RecognitionMethod::annotation(Some("test".into())),
            },
        );
        assert_eq!(deny.len(), 2);
        assert!(deny.contains("secret"));
        let rule = deny.get("other").unwrap();
        assert_eq!(rule.category, EntityCategory::Financial);
        assert_eq!(
            rule.method,
            RecognitionMethod::annotation(Some("test".into()))
        );
    }

    #[test]
    fn column_confidence_raw() {
        let engine = PatternEngine::instance();
        let matches = engine.scan_raw("I paid in US Dollar and also in USD.", &empty_ctx());
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
    fn into_entity_preserves_fields() {
        let raw = RawMatch {
            pattern_name: Some("ssn".into()),
            category: EntityCategory::PersonalIdentity,
            entity_kind: EntityKind::GovernmentId,
            value: "123-45-6789".into(),
            start: 5,
            end: 16,
            confidence: 0.9,
            recognition_methods: vec![RecognitionMethod::regex_validated("ssn", "ssn")],
            context: None,
        };
        let entity = raw.into_entity();
        assert_eq!(entity.text_value(), Some("123-45-6789"));
        assert_eq!(entity.entity_kind, EntityKind::GovernmentId);
        assert_eq!(
            entity.recognition_methods,
            vec![RecognitionMethod::regex_validated("ssn", "ssn")]
        );
        assert!((entity.confidence - 0.9).abs() < f64::EPSILON);
        assert!(entity.location.as_text().is_some());
    }
}
