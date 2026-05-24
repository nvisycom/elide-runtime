//! Pre-compiled pattern matching engine.
//!
//! [`PatternEngine`] compiles all built-in (and optionally user-selected)
//! regex patterns and dictionary automata into a single unit that can
//! scan text in one call. Use [`PatternEngine::builder`] for configuration
//! or [`PatternEngine::instance`] for an out-of-the-box singleton.
//!
//! # Layout
//!
//! - The engine type and its scan entrypoints live at the top level
//!   (this module).
//! - [`PatternEngineBuilder`] / [`PatternEngineError`] are in
//!   sibling files.
//! - [`filter`] groups the per-scan inputs callers configure:
//!   [`AllowList`], [`DenyList`], [`ContextHint`], and the
//!   [`ScanContext`] that bundles them.
//! - `scan` (crate-private) holds the internal matching machinery:
//!   compiled per-pattern entries, the `RawMatch` exchange type,
//!   the per-phase scan logic, overlap-aware dedup, and the
//!   context-aware `ContextEnhancer`.

mod builder;
mod error;
mod pattern_filter;

pub mod filter;
pub(crate) mod scan;

use std::fmt;
use std::sync::LazyLock;

use nvisy_ontology::entity::Entity;
use regex::RegexSet;

pub use self::builder::PatternEngineBuilder;
pub use self::error::{ExtraPatternError, PatternEngineError};
pub use self::filter::ScanContext;
pub use self::pattern_filter::PatternFilter;
use self::scan::dedup::{dedup_overlapping, sort_for_dedup};
use self::scan::enhancer::ContextEnhancer;
use self::scan::entries::{CompiledPattern, DictEntry, GlobEntry, RegexEntry};
use self::scan::pattern_match::RawMatch;
use self::scan::phases::{scan_deny_list, scan_dict, scan_glob, scan_regex};
use crate::patterns::{Pattern, PatternCompile, RuntimePattern};
use crate::validators::ValidatorResolver;

const TARGET: &str = "nvisy_pattern::engine";

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
    pub(in crate::engine) regex_set: RegexSet,
    pub(in crate::engine) regex_entries: Vec<RegexEntry>,
    pub(in crate::engine) glob_entries: Vec<GlobEntry>,
    pub(in crate::engine) dict_entries: Vec<DictEntry>,
    pub(in crate::engine) validators: ValidatorResolver,
    pub(in crate::engine) confidence_threshold: f64,
}

impl fmt::Debug for PatternEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PatternEngine")
            .field("regex_patterns", &self.regex_entries.len())
            .field("glob_patterns", &self.glob_entries.len())
            .field("dict_patterns", &self.dict_entries.len())
            .field("confidence_threshold", &self.confidence_threshold)
            .finish()
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

    /// Scan `text` and return detected entities with [`TextLocation`]s.
    ///
    /// Matches whose value appears in the allow list are suppressed.
    /// Deny-list values found in the text are injected as synthetic
    /// matches with confidence `1.0` when not already matched.
    ///
    /// Each entity carries a [`TextLocation`] with `start_offset` and
    /// `end_offset` set from the match.
    ///
    /// [`TextLocation`]: nvisy_ontology::entity::TextLocation
    #[tracing::instrument(level = "trace", target = TARGET, skip(self, text, ctx), fields(text_len = text.len(), entities = tracing::field::Empty))]
    pub fn scan_entities(&self, text: &str, ctx: &ScanContext) -> Vec<Entity> {
        let mut raw = self.scan_raw(text, ctx);

        // Apply context-based confidence adjustment before threshold
        // filtering so a match just below threshold can be promoted by
        // keyword co-occurrence.
        ContextEnhancer::new(text, &ctx.hints).enhance(&mut raw);

        sort_for_dedup(&mut raw);
        let deduped = dedup_overlapping(&raw);

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

    /// Validate that every `RuntimePattern` compiles cleanly against
    /// this engine's matcher backends. Returns one
    /// [`ExtraPatternError`] per malformed pattern. Use this before a
    /// scan if you want to surface bad patterns to the caller —
    /// during [`Self::scan_entities`] compile errors on `extra_patterns`
    /// are silently dropped.
    ///
    /// [`ExtraPatternError`]: crate::ExtraPatternError
    pub fn validate_runtime_patterns(&self, patterns: &[RuntimePattern]) -> Vec<ExtraPatternError> {
        let dict_lookup = |name: &str| -> Option<&dyn crate::dictionaries::Dictionary> {
            crate::dictionaries::builtin_registry().get(name)
        };
        patterns
            .iter()
            .filter_map(|p| match p.compile_with(&dict_lookup) {
                Ok(_) => None,
                Err(source) => Some(ExtraPatternError {
                    name: p.name().to_owned(),
                    source,
                }),
            })
            .collect()
    }

    /// Internal: scan and return raw matches.
    pub(in crate::engine) fn scan_raw(&self, text: &str, ctx: &ScanContext) -> Vec<RawMatch> {
        let mut results = Vec::new();

        let candidates = self.regex_set.matches(text).into_iter();
        scan_regex(
            candidates,
            &self.regex_entries,
            &self.validators,
            text,
            &ctx.allow,
            &mut results,
        );
        scan_glob(&self.glob_entries, text, &ctx.allow, &mut results);
        scan_dict(&self.dict_entries, text, &ctx.allow, &mut results);
        scan_deny_list(text, &ctx.deny, &mut results);

        if !ctx.extra_patterns.is_empty() {
            self.scan_extra_patterns(text, ctx, &mut results);
        }

        results
    }

    /// Compile + scan `ctx.extra_patterns` on the hot path. Compile
    /// failures are dropped silently (logged at TRACE) — operators
    /// who need them surfaced should call
    /// [`Self::validate_runtime_patterns`] before scanning.
    fn scan_extra_patterns(&self, text: &str, ctx: &ScanContext, results: &mut Vec<RawMatch>) {
        let dict_lookup = |name: &str| -> Option<&dyn crate::dictionaries::Dictionary> {
            crate::dictionaries::builtin_registry().get(name)
        };

        let mut extra_regex_entries = Vec::new();
        let mut extra_regex_sources = Vec::new();
        let mut extra_glob_entries = Vec::new();
        let mut extra_dict_entries = Vec::new();

        for p in &ctx.extra_patterns {
            match p.compile_with(&dict_lookup) {
                Ok(Some(CompiledPattern::Regex {
                    entry,
                    regex_source,
                })) => {
                    extra_regex_sources.push(regex_source);
                    extra_regex_entries.push(entry);
                }
                Ok(Some(CompiledPattern::Glob(entry))) => extra_glob_entries.push(entry),
                Ok(Some(CompiledPattern::Dictionary(entry))) => extra_dict_entries.push(entry),
                Ok(None) => {}
                Err(source) => tracing::trace!(
                    target: TARGET,
                    pattern = p.name(),
                    error = %source,
                    "skipped extra_pattern: compile failed",
                ),
            }
        }

        if !extra_regex_entries.is_empty() {
            match RegexSet::new(&extra_regex_sources) {
                Ok(set) => {
                    let candidates = set.matches(text).into_iter();
                    scan_regex(
                        candidates,
                        &extra_regex_entries,
                        &self.validators,
                        text,
                        &ctx.allow,
                        results,
                    );
                }
                Err(source) => tracing::trace!(
                    target: TARGET,
                    error = %source,
                    "skipped extra_patterns: RegexSet build failed",
                ),
            }
        }

        scan_glob(&extra_glob_entries, text, &ctx.allow, results);
        scan_dict(&extra_dict_entries, text, &ctx.allow, results);
    }
}

static DEFAULT_ENGINE: LazyLock<PatternEngine> = LazyLock::new(|| {
    PatternEngine::builder()
        .build()
        .expect("built-in patterns must compile")
});

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{EntityCategory, EntityKind, RecognitionMethod};

    use super::filter::{DenyList, DenyRule};
    use super::scan::dedup::{beats, dedup_overlapping, sort_for_dedup, spans_overlap};
    use super::scan::pattern_match::RawMatch;
    use super::*;
    use crate::patterns::{GlobPattern, MatchSource, RegexPattern, RuntimePattern};

    fn empty_ctx() -> ScanContext {
        ScanContext::default()
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
        let ctx = ScanContext {
            allow: ["123-45-6789"].into_iter().collect(),
            ..Default::default()
        };
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
        let mut deny = DenyList::new();
        deny.insert(
            "secret-value-42",
            DenyRule {
                category: EntityCategory::PersonalIdentity,
                entity_kind: EntityKind::PersonName,
            },
        );
        let engine = PatternEngine::builder()
            .with_patterns(&["email"])
            .build()
            .unwrap();
        let ctx = ScanContext {
            deny,
            ..Default::default()
        };
        let matches = engine.scan_raw("The secret-value-42 should be detected.", &ctx);
        let deny_match = matches
            .iter()
            .find(|m| m.pattern_name.is_none())
            .expect("deny list value should be injected");
        assert_eq!(deny_match.value, "secret-value-42");
        assert_eq!(deny_match.confidence, 1.0);
        assert_eq!(deny_match.entity_kind, EntityKind::PersonName);
        assert_eq!(
            deny_match.recognition_methods.as_slice(),
            &[RecognitionMethod::deny_list()],
        );
    }

    #[test]
    fn deny_list_not_injected_when_absent() {
        let mut deny = DenyList::new();
        deny.insert(
            "not-in-text",
            DenyRule {
                category: EntityCategory::PersonalIdentity,
                entity_kind: EntityKind::PersonName,
            },
        );
        let engine = PatternEngine::builder()
            .with_patterns(&["email"])
            .build()
            .unwrap();
        let ctx = ScanContext {
            deny,
            ..Default::default()
        };
        let matches = engine.scan_raw("Nothing special here.", &ctx);
        assert!(
            !matches.iter().any(|m| m.pattern_name.is_none()),
            "deny list value not in text should not be injected"
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
    fn dedup_prefers_tighter_higher_confidence_match() {
        let wide = RawMatch {
            pattern_name: Some("wide".into()),
            category: EntityCategory::PersonalIdentity,
            entity_kind: EntityKind::GovernmentId,
            value: "wide".into(),
            start: 10,
            end: 30,
            confidence: 0.7,
            recognition_methods: smallvec::smallvec![RecognitionMethod::regex("wide")],
            context: None,
        };
        let tight = RawMatch {
            pattern_name: Some("tight".into()),
            category: EntityCategory::PersonalIdentity,
            entity_kind: EntityKind::GovernmentId,
            value: "tight".into(),
            start: 15,
            end: 20,
            confidence: 0.9,
            recognition_methods: smallvec::smallvec![RecognitionMethod::regex("tight")],
            context: None,
        };
        let mut raw = vec![wide, tight];
        sort_for_dedup(&mut raw);
        let kept = dedup_overlapping(&raw);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].pattern_name.as_deref(), Some("tight"));
    }

    #[test]
    fn spans_overlap_basic() {
        assert!(spans_overlap(0, 10, 5, 15));
        assert!(spans_overlap(5, 15, 0, 10));
        assert!(!spans_overlap(0, 5, 5, 10));
        assert!(!spans_overlap(5, 10, 0, 5));
    }

    #[test]
    fn beats_prefers_higher_confidence_then_tighter_span() {
        let mk = |start, end, conf| RawMatch {
            pattern_name: None,
            category: EntityCategory::PersonalIdentity,
            entity_kind: EntityKind::PersonName,
            value: "x".into(),
            start,
            end,
            confidence: conf,
            recognition_methods: smallvec::smallvec![],
            context: None,
        };
        let high = mk(0, 10, 0.9);
        let low = mk(0, 10, 0.5);
        assert!(beats(&high, &low, true));
        assert!(!beats(&low, &high, true));

        let tight = mk(0, 5, 0.7);
        let wide = mk(0, 10, 0.7);
        assert!(beats(&tight, &wide, true));
        assert!(!beats(&wide, &tight, true));

        let a = mk(0, 5, 0.7);
        let b = mk(0, 5, 0.7);
        assert!(beats(&a, &b, true));
        assert!(!beats(&a, &b, false));
    }

    #[test]
    fn glob_extra_pattern_matches_token() {
        let engine = PatternEngine::builder()
            .with_patterns(&["email"])
            .build()
            .unwrap();
        let ctx = ScanContext {
            extra_patterns: vec![
                RuntimePattern::new(
                    "internal-invoice",
                    MatchSource::Glob(GlobPattern {
                        glob: "INV-*".into(),
                        case_sensitive: true,
                        confidence: 0.8,
                    }),
                )
                .with_category(EntityCategory::Financial)
                .with_kind(EntityKind::PaymentCard),
            ],
            ..Default::default()
        };
        let matches = engine.scan_raw("Please review INV-12345, attached above.", &ctx);
        let m = matches
            .iter()
            .find(|m| m.pattern_name.as_deref() == Some("internal-invoice"))
            .expect("glob extra_pattern should match");
        assert_eq!(m.value, "INV-12345");
        assert_eq!(m.confidence, 0.8);
    }

    #[test]
    fn glob_extra_pattern_does_not_match_non_token() {
        let engine = PatternEngine::builder()
            .with_patterns(&["email"])
            .build()
            .unwrap();
        let ctx = ScanContext {
            extra_patterns: vec![
                RuntimePattern::new(
                    "x",
                    MatchSource::Glob(GlobPattern {
                        glob: "INV-*".into(),
                        case_sensitive: true,
                        confidence: 1.0,
                    }),
                )
                .with_category(EntityCategory::Financial)
                .with_kind(EntityKind::PaymentCard),
            ],
            ..Default::default()
        };
        // "XINV-1" is one token but does not start with "INV-".
        let matches = engine.scan_raw("XINV-1 here", &ctx);
        assert!(
            !matches
                .iter()
                .any(|m| m.pattern_name.as_deref() == Some("x")),
            "glob is anchored per-token; XINV-1 must not match INV-*",
        );
    }

    #[test]
    fn glob_extra_pattern_case_insensitive() {
        let engine = PatternEngine::builder()
            .with_patterns(&["email"])
            .build()
            .unwrap();
        let ctx = ScanContext {
            extra_patterns: vec![
                RuntimePattern::new(
                    "ci",
                    MatchSource::Glob(GlobPattern {
                        glob: "INV-*".into(),
                        case_sensitive: false,
                        confidence: 1.0,
                    }),
                )
                .with_category(EntityCategory::Financial)
                .with_kind(EntityKind::PaymentCard),
            ],
            ..Default::default()
        };
        let matches = engine.scan_raw("see inv-7 attached", &ctx);
        assert!(
            matches
                .iter()
                .any(|m| m.pattern_name.as_deref() == Some("ci")),
            "case_sensitive=false glob should match lowercase token",
        );
    }

    #[test]
    fn malformed_runtime_pattern_surfaces_via_validate() {
        let engine = PatternEngine::builder()
            .with_patterns(&["email"])
            .build()
            .unwrap();
        let bad = RuntimePattern::new(
            "bad",
            MatchSource::Regex(RegexPattern {
                regex: r"(unclosed".into(),
                validator: None,
                case_sensitive: true,
                confidence: 1.0,
            }),
        );
        let errors = engine.validate_runtime_patterns(&[bad]);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].name, "bad");
    }

    #[test]
    fn extra_regex_pattern_compiles_and_matches() {
        let engine = PatternEngine::builder()
            .with_patterns(&["email"])
            .build()
            .unwrap();
        let ctx = ScanContext {
            extra_patterns: vec![
                RuntimePattern::new(
                    "emp-id",
                    MatchSource::Regex(RegexPattern {
                        regex: r"\bEMP-\d{4}\b".into(),
                        validator: None,
                        case_sensitive: true,
                        confidence: 0.9,
                    }),
                )
                .with_category(EntityCategory::PersonalIdentity)
                .with_kind(EntityKind::PersonName),
            ],
            ..Default::default()
        };
        let matches = engine.scan_raw("Hand off to EMP-4242 today.", &ctx);
        let m = matches
            .iter()
            .find(|m| m.pattern_name.as_deref() == Some("emp-id"))
            .expect("regex extra_pattern should match");
        assert_eq!(m.value, "EMP-4242");
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
            recognition_methods: smallvec::smallvec![RecognitionMethod::regex_validated(
                "ssn", "ssn"
            )],
            context: None,
        };
        let entity = raw.into_entity();
        let loc = entity.location.as_text().unwrap();
        assert_eq!(loc.start_offset, 5);
        assert_eq!(loc.end_offset, 16);
        assert_eq!(entity.entity_kind, EntityKind::GovernmentId);
        assert_eq!(
            entity.recognition_methods,
            vec![RecognitionMethod::regex_validated("ssn", "ssn")]
        );
        assert!((entity.confidence.get() - 0.9).abs() < f64::EPSILON);
        assert!(entity.location.as_text().is_some());
    }
}
