//! Integration tests for the pattern engine's public scan API.

use nvisy_ontology::entity::EntityKind;
use nvisy_pattern::{PatternEngine, ScanContext};

fn empty_ctx() -> ScanContext {
    ScanContext::default()
}

#[test]
fn finds_ssn() {
    let engine = PatternEngine::instance();
    let entities = engine.scan_entities("My SSN is 123-45-6789.", &empty_ctx());
    assert!(
        entities
            .iter()
            .any(|e| e.entity_kind == EntityKind::GovernmentId),
        "expected SSN entity, got: {:?}",
        entities.iter().map(|e| e.entity_kind).collect::<Vec<_>>()
    );
}

#[test]
fn finds_email() {
    let engine = PatternEngine::instance();
    let entities = engine.scan_entities("Contact: alice@example.com", &empty_ctx());
    assert!(
        entities
            .iter()
            .any(|e| e.entity_kind == EntityKind::EmailAddress),
        "expected email entity, got: {:?}",
        entities.iter().map(|e| e.entity_kind).collect::<Vec<_>>()
    );
}

#[test]
fn confidence_threshold_filters() {
    let engine = PatternEngine::builder()
        .with_confidence_threshold(0.99)
        .build()
        .unwrap();
    let entities = engine.scan_entities("number 123-45-6789 here", &empty_ctx());
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

    let with_keyword = engine.scan_entities("SSN: 123-45-6789", &empty_ctx());
    let without_keyword = engine.scan_entities("number 123-45-6789 here", &empty_ctx());

    let boosted = with_keyword
        .iter()
        .find(|e| e.entity_kind == EntityKind::GovernmentId)
        .expect("should detect SSN with keyword");
    let unboosted = without_keyword
        .iter()
        .find(|e| e.entity_kind == EntityKind::GovernmentId)
        .expect("should detect SSN without keyword");

    assert!(
        boosted.confidence > unboosted.confidence,
        "confidence with keyword ({}) should exceed without ({})",
        boosted.confidence,
        unboosted.confidence,
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
    let all_entities = all.scan_entities(text, &empty_ctx());
    let ssn_entities = ssn_only.scan_entities(text, &empty_ctx());

    assert!(
        all_entities.len() > ssn_entities.len(),
        "all patterns ({}) should find more than ssn-only ({})",
        all_entities.len(),
        ssn_entities.len(),
    );
    assert!(ssn_entities
        .iter()
        .all(|e| e.entity_kind == EntityKind::GovernmentId));
}

#[test]
fn confidence_threshold_filters_after_boost() {
    let engine = PatternEngine::builder()
        .with_patterns(&["ssn"])
        .with_confidence_threshold(0.95)
        .build()
        .unwrap();

    let with_keyword = engine.scan_entities("SSN: 123-45-6789", &empty_ctx());
    let without_keyword = engine.scan_entities("number 123-45-6789 here", &empty_ctx());

    assert!(
        !with_keyword.is_empty(),
        "boosted match should survive high threshold"
    );
    assert!(
        without_keyword.is_empty(),
        "unboosted match should be filtered by high threshold"
    );
}

#[test]
fn dictionary_matches_are_found() {
    use nvisy_ontology::entity::RecognitionMethod;

    let engine = PatternEngine::instance();
    let entities = engine.scan_entities("She is American and speaks English.", &empty_ctx());
    assert!(
        entities
            .iter()
            .any(|e| e.recognition_methods.contains(&RecognitionMethod::Dictionary)),
        "expected dictionary match, got: {:?}",
        entities
            .iter()
            .map(|e| &e.recognition_methods)
            .collect::<Vec<_>>()
    );
}

#[test]
fn column_confidence_applies_to_csv_dictionaries() {
    let engine = PatternEngine::instance();
    let entities = engine.scan_entities("I paid in US Dollar and also in USD.", &empty_ctx());
    let full_name = entities.iter().find(|e| e.value == "US Dollar");
    let code = entities.iter().find(|e| e.value == "USD");
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
fn scan_entities_returns_entities_with_location() {
    let engine = PatternEngine::builder()
        .with_patterns(&["ssn"])
        .build()
        .unwrap();
    let entities = engine.scan_entities("SSN: 123-45-6789", &empty_ctx());
    assert!(!entities.is_empty());
    let e = &entities[0];
    assert_eq!(e.entity_kind, EntityKind::GovernmentId);
    assert!(e.location.is_some());
}
