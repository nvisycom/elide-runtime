//! Integration tests for entity detection via the public scan API.

use nvisy_ontology::entity::{EntityKind, PatternKind, RecognitionMethod};
use nvisy_pattern::PatternEngine;
use nvisy_pattern::filter::PatternContext;

fn empty_ctx() -> PatternContext {
    PatternContext::default()
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
fn dictionary_matches_are_found() {
    let engine = PatternEngine::instance();
    let entities = engine.scan_entities("She is American and speaks English.", &empty_ctx());
    assert!(
        entities
            .iter()
            .any(|e| e.recognition_methods.iter().any(|m| matches!(
                m,
                RecognitionMethod::Pattern(p) if p.kind == PatternKind::Dictionary,
            ))),
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
    // "US Dollar" at 10..19, "USD" at 32..35
    let full_name = entities
        .iter()
        .find(|e| e.location.as_text().is_some_and(|l| l.start_offset == 10));
    let code = entities
        .iter()
        .find(|e| e.location.as_text().is_some_and(|l| l.start_offset == 32));
    assert!(full_name.is_some(), "should match 'US Dollar'");
    assert!(code.is_some(), "should match 'USD'");
    let full_conf = full_name.unwrap().confidence.get();
    let code_conf = code.unwrap().confidence.get();
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
    assert!(e.location.as_text().is_some());
}
