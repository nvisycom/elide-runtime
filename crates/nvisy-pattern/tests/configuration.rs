//! Integration tests for engine configuration: threshold, boost, pattern selection.

use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::primitive::ConfidenceThreshold;
use nvisy_pattern::PatternEngine;
use nvisy_pattern::filter::PatternContext;

fn empty_ctx() -> PatternContext {
    PatternContext::default()
}

#[test]
fn confidence_threshold_filters() {
    let engine = PatternEngine::builder()
        .with_confidence_threshold(ConfidenceThreshold::clamped(0.99))
        .build()
        .unwrap();
    let entities = engine.scan_text("number 123-45-6789 here", &empty_ctx());
    assert!(
        !entities
            .iter()
            .any(|e| e.entity_kind == EntityKind::GovernmentId),
        "SSN should be filtered by 0.99 threshold"
    );
}

#[test]
fn context_boost_applied_when_keyword_present() {
    let engine = PatternEngine::builder()
        .with_patterns(&["ssn"])
        .build()
        .unwrap();

    let with_keyword = engine.scan_text("SSN: 123-45-6789", &empty_ctx());
    let without_keyword = engine.scan_text("number 123-45-6789 here", &empty_ctx());

    let boosted = with_keyword
        .iter()
        .find(|e| e.entity_kind == EntityKind::GovernmentId)
        .expect("should detect SSN with keyword");
    let unboosted = without_keyword
        .iter()
        .find(|e| e.entity_kind == EntityKind::GovernmentId)
        .expect("should detect SSN without keyword");

    let boosted_conf = boosted.confidence.get();
    let unboosted_conf = unboosted.confidence.get();
    assert!(
        boosted_conf > unboosted_conf,
        "confidence with keyword ({boosted_conf}) should exceed without ({unboosted_conf})",
    );
}

#[test]
fn pattern_selection_restricts_results() {
    let all = PatternEngine::instance();
    let ssn_only = PatternEngine::builder()
        .with_patterns(&["ssn"])
        .build()
        .unwrap();

    let text = "SSN: 123-45-6789, email: alice@example.com";
    let all_entities = all.scan_text(text, &empty_ctx());
    let ssn_entities = ssn_only.scan_text(text, &empty_ctx());

    assert!(
        all_entities.len() > ssn_entities.len(),
        "all patterns ({}) should find more than ssn-only ({})",
        all_entities.len(),
        ssn_entities.len(),
    );
    assert!(
        ssn_entities
            .iter()
            .all(|e| e.entity_kind == EntityKind::GovernmentId)
    );
}

#[test]
fn confidence_threshold_filters_after_boost() {
    let engine = PatternEngine::builder()
        .with_patterns(&["ssn"])
        .with_confidence_threshold(ConfidenceThreshold::clamped(0.95))
        .build()
        .unwrap();

    let with_keyword = engine.scan_text("SSN: 123-45-6789", &empty_ctx());
    let without_keyword = engine.scan_text("number 123-45-6789 here", &empty_ctx());

    assert!(
        !with_keyword.is_empty(),
        "boosted match should survive high threshold"
    );
    assert!(
        without_keyword.is_empty(),
        "unboosted match should be filtered by high threshold"
    );
}
