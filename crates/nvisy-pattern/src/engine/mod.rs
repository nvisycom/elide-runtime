//! Pre-compiled pattern matching engine.
//!
//! [`PatternEngine`] compiles all built-in (and optionally user-selected)
//! regex patterns and dictionary automata into a single unit that can
//! scan text in one call. Use [`PatternEngine::builder`] for configuration
//! or [`PatternEngine::instance`] for an out-of-the-box singleton.
//!
//! # Layout
//!
//! - The engine type, builder, and structured build error live at the
//!   top level (this module).
//! - [`filter`] groups the per-scan inputs callers configure:
//!   [`AllowList`], [`DenyList`], and the [`ScanContext`] that bundles
//!   them.
//! - [`scan`] holds the internal matching machinery: compiled per-pattern
//!   entries, the [`RawMatch`](scan::pattern_match::RawMatch) exchange
//!   type, the per-phase scan logic, and overlap-aware dedup.

mod builder;
mod error;
mod pattern_engine;

pub mod filter;
pub(crate) mod scan;

pub use self::builder::PatternEngineBuilder;
pub use self::error::PatternEngineError;
pub use self::filter::{AllowList, DenyList, DenyRule, ScanContext};
pub use self::pattern_engine::PatternEngine;

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{EntityCategory, EntityKind, ModelKind, RecognitionMethod};

    use super::scan::dedup::{beats, dedup_overlapping, sort_for_dedup, spans_overlap};
    use super::scan::pattern_match::RawMatch;
    use super::*;

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
                method: RecognitionMethod::ner("test", ModelKind::SelfHosted),
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
            &[RecognitionMethod::ner("test", ModelKind::SelfHosted)]
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
                method: RecognitionMethod::annotation(Some("test".into())),
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
        assert!((entity.confidence - 0.9).abs() < f64::EPSILON);
        assert!(entity.location.as_text().is_some());
    }
}
