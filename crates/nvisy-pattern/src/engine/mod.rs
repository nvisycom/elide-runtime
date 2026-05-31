//! Pre-compiled pattern matching engine.
//!
//! [`PatternEngine`] compiles all built-in (and optionally
//! user-selected) regex patterns and dictionary automata into a
//! single unit that can scan text in one call. Use
//! [`PatternEngine::builder`] for configuration or
//! [`PatternEngine::instance`] for an out-of-the-box singleton.
//!
//! # Layout
//!
//! - The engine type and its scan entrypoints live at the top level
//!   (this module).
//! - [`PatternEngineBuilder`] / [`PatternEngineError`] are in
//!   sibling files.
//! - [`filter`] groups the per-scan inputs callers configure:
//!   [`AllowList`], [`DenyList`], [`ContextHint`], and the
//!   [`PatternContext`] that bundles them.
//! - `scan` (crate-private) holds the internal matching machinery:
//!   compiled per-pattern entries, the `EntityCandidate` exchange
//!   type, the per-phase scan logic, and the context-aware
//!   `ContextEnhancer`. Cross-recognizer deduplication is the
//!   engine layer's responsibility, not this crate's.

mod builder;
mod error;
mod pattern_filter;

pub mod filter;
pub(crate) mod scan;

use std::fmt;
use std::sync::LazyLock;

use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Text;
use regex::RegexSet;

pub use self::builder::PatternEngineBuilder;
pub use self::error::{ExtraPatternError, PatternEngineError};
pub use self::filter::PatternContext;
pub use self::pattern_filter::PatternFilter;
use self::scan::candidate::EntityCandidate;
use self::scan::enhancer::ContextEnhancer;
use self::scan::entries::{CompiledBuckets, DictEntry, RegexEntry};
use self::scan::phases::{scan_deny_list, scan_dict, scan_regex};
use crate::dictionaries::{self, Dictionary};
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
/// Allow-list filtering is applied inline during phases 1-2.
///
/// Build via [`PatternEngine::builder`] or use [`PatternEngine::instance`]
/// for the singleton with all built-in patterns.
pub struct PatternEngine {
    pub(in crate::engine) regex_set: RegexSet,
    pub(in crate::engine) regex_entries: Vec<RegexEntry>,
    pub(in crate::engine) dict_entries: Vec<DictEntry>,
    pub(in crate::engine) validators: ValidatorResolver,
}

impl fmt::Debug for PatternEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PatternEngine")
            .field("regex_patterns", &self.regex_entries.len())
            .field("dict_patterns", &self.dict_entries.len())
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

    /// Scan a bare `&str` for matches. The engine layer (in
    /// `nvisy-engine`) drives block-aware scanning by walking each
    /// block's text through this method and lifting the returned
    /// offsets back to source coordinates via the block's spans.
    pub fn scan_text(&self, text: &str, ctx: &PatternContext) -> Vec<Entity<Text>> {
        let mut candidates = self.scan_raw(text, ctx);
        ContextEnhancer::new(text, &ctx.hints).enhance(&mut candidates);
        candidates.into_iter().map(|c| c.entity).collect()
    }

    /// Validate that every `RuntimePattern` compiles cleanly against
    /// this engine's matcher backends. Returns one
    /// [`ExtraPatternError`] per malformed pattern. Use this before
    /// a scan if you want to surface bad patterns to the caller —
    /// during [`Self::scan_text`] compile errors on `extra_patterns`
    /// are silently dropped.
    ///
    /// [`ExtraPatternError`]: crate::ExtraPatternError
    pub fn validate_patterns(&self, patterns: &[RuntimePattern]) -> Vec<ExtraPatternError> {
        patterns
            .iter()
            .filter_map(|p| match p.compile_with(&builtin_dict_lookup) {
                Ok(_) => None,
                Err(source) => Some(ExtraPatternError {
                    name: p.name().to_owned(),
                    source,
                }),
            })
            .collect()
    }

    /// Scan and return raw matches from every phase plus any
    /// caller-supplied extras.
    fn scan_raw(&self, text: &str, ctx: &PatternContext) -> Vec<EntityCandidate> {
        let mut results = scan_regex(
            self.regex_set.matches(text),
            &self.regex_entries,
            &self.validators,
            text,
            &ctx.allow,
        );
        results.extend(scan_dict(&self.dict_entries, text, &ctx.allow));
        let deny_hits = scan_deny_list(text, &ctx.deny, &results);
        results.extend(deny_hits);
        if !ctx.extra_patterns.is_empty() {
            results.extend(self.scan_extra_patterns(text, ctx));
        }
        results
    }

    /// Compile + scan `ctx.extra_patterns` on the hot path. Compile
    /// failures are dropped silently (logged at TRACE) — operators
    /// who need them surfaced should call
    /// [`Self::validate_patterns`] before scanning.
    fn scan_extra_patterns(&self, text: &str, ctx: &PatternContext) -> Vec<EntityCandidate> {
        let mut buckets = CompiledBuckets::default();
        for p in &ctx.extra_patterns {
            match p.compile_with(&builtin_dict_lookup) {
                Ok(Some(compiled)) => buckets.insert(compiled),
                Ok(None) => {}
                Err(source) => tracing::trace!(
                    target: TARGET,
                    pattern = p.name(),
                    error = %source,
                    "skipped extra_pattern: compile failed",
                ),
            }
        }
        let compiled = match buckets.finish() {
            Ok(c) => c,
            Err(source) => {
                tracing::trace!(
                    target: TARGET,
                    error = %source,
                    "skipped extra_patterns: prefilter build failed",
                );
                return Vec::new();
            }
        };
        let mut results = scan_regex(
            compiled.regex_set.matches(text),
            &compiled.regex_entries,
            &self.validators,
            text,
            &ctx.allow,
        );
        results.extend(scan_dict(&compiled.dict_entries, text, &ctx.allow));
        results
    }
}

/// Dictionary lookup for the hot-path extras compile — consults only
/// the process-wide builtin registry. The builder supplies a richer
/// lookup that also overlays user-loaded dirs.
fn builtin_dict_lookup(name: &str) -> Option<&'static dyn Dictionary> {
    dictionaries::builtin_registry().get(name)
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
    use super::scan::candidate::EntityCandidate;
    use super::*;
    use crate::patterns::{MatchSource, RegexPattern, RuntimePattern};

    fn empty_ctx() -> PatternContext {
        PatternContext::default()
    }

    /// Pull the pattern name from a candidate's first
    /// `RecognitionMethod::Pattern` provenance, or `None` for
    /// deny-list / no-provenance candidates.
    fn pattern_name(c: &EntityCandidate) -> Option<&str> {
        c.entity.recognition_methods.iter().find_map(|m| match m {
            RecognitionMethod::Pattern(p) => p.name(),
            _ => None,
        })
    }

    /// Span of a candidate's text location.
    fn span(c: &EntityCandidate) -> (usize, usize) {
        (c.entity.location.start, c.entity.location.end)
    }

    #[test]
    fn scan_raw_returns_correct_offsets() {
        let engine = PatternEngine::instance();
        let text = "SSN: 123-45-6789";
        let matches = engine.scan_raw(text, &empty_ctx());
        let ssn = matches
            .iter()
            .find(|m| pattern_name(m) == Some("ssn"))
            .unwrap();
        let (start, end) = span(ssn);
        assert_eq!(&text[start..end], "123-45-6789");
    }

    #[test]
    fn allow_list_suppresses_match() {
        let engine = PatternEngine::builder()
            .with_patterns(&["ssn"])
            .build()
            .unwrap();
        let ctx = PatternContext {
            allow: ["123-45-6789"].into_iter().collect(),
            ..Default::default()
        };
        let matches = engine.scan_raw("SSN: 123-45-6789", &ctx);
        assert!(
            !matches.iter().any(|m| pattern_name(m) == Some("ssn")),
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
        let ctx = PatternContext {
            deny,
            ..Default::default()
        };
        let text = "The secret-value-42 should be detected.";
        let matches = engine.scan_raw(text, &ctx);
        let deny_match = matches
            .iter()
            .find(|m| pattern_name(m).is_none())
            .expect("deny list value should be injected");
        let (start, end) = span(deny_match);
        assert_eq!(&text[start..end], "secret-value-42");
        assert_eq!(deny_match.entity.confidence.get(), 1.0);
        assert_eq!(deny_match.entity.entity_kind, EntityKind::PersonName);
        assert_eq!(
            deny_match.entity.recognition_methods,
            vec![RecognitionMethod::deny_list()],
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
        let ctx = PatternContext {
            deny,
            ..Default::default()
        };
        let matches = engine.scan_raw("Nothing special here.", &ctx);
        assert!(
            !matches.iter().any(|m| pattern_name(m).is_none()),
            "deny list value not in text should not be injected"
        );
    }

    #[test]
    fn column_confidence_raw() {
        let engine = PatternEngine::instance();
        let text = "I paid in US Dollar and also in USD.";
        let matches = engine.scan_raw(text, &empty_ctx());
        let value_of = |c: &EntityCandidate| {
            let (s, e) = span(c);
            text[s..e].to_owned()
        };
        let full_name = matches.iter().find(|m| value_of(m) == "US Dollar");
        let code = matches.iter().find(|m| value_of(m) == "USD");
        assert!(full_name.is_some(), "should match 'US Dollar'");
        assert!(code.is_some(), "should match 'USD'");
        let full_conf = full_name.unwrap().entity.confidence.get();
        let code_conf = code.unwrap().entity.confidence.get();
        assert!(
            full_conf > code_conf,
            "full name confidence ({full_conf}) should exceed code confidence ({code_conf})"
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
        let errors = engine.validate_patterns(&[bad]);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].name, "bad");
    }

    #[test]
    fn extra_regex_pattern_compiles_and_matches() {
        let engine = PatternEngine::builder()
            .with_patterns(&["email"])
            .build()
            .unwrap();
        let ctx = PatternContext {
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
        let text = "Hand off to EMP-4242 today.";
        let matches = engine.scan_raw(text, &ctx);
        let m = matches
            .iter()
            .find(|m| pattern_name(m) == Some("emp-id"))
            .expect("regex extra_pattern should match");
        let (s, e) = span(m);
        assert_eq!(&text[s..e], "EMP-4242");
    }
}
