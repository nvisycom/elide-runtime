//! End-to-end: feed real input through the
//! recognizer → [`ContextEnhancer`] handoff, and verify that
//! confidence is boosted, the recognition step's `contextual` flag is
//! set, and a [`Refinement`]
//! step is appended only for matches that had a nearby keyword.
//!
//! [`Refinement`]: nvisy_core::entity::TrailStepKind::Refinement

use nvisy_core::context::{Context, ContextEnhancer};
use nvisy_core::entity::{PatternProvenance, TrailProvenance, TrailStepKind, builtins};
use nvisy_core::modality::TextData;
use nvisy_core::primitive::Confidence;
use nvisy_core::recognition::{EntityRecognizer, RecognizerInput};
use nvisy_pattern::{PatternRecognizer, PatternRegistry, Regex};

#[tokio::test]
async fn enhancer_boosts_matches_near_keyword_only() {
    let ssn = Regex::builder()
        .with_name("ssn")
        .with_label(builtins::GOVERNMENT_ID.label_ref())
        .with_regex(r"\b\d{3}-\d{2}-\d{4}\b")
        .with_score(Confidence::clamped(0.6))
        .with_context(Context::new(["ssn", "social security"]))
        .build()
        .expect("ssn regex builds");

    let registry = PatternRegistry::new().with_pattern(ssn);
    let recognizer = PatternRecognizer::builder()
        .with_registry(registry.clone())
        .build()
        .expect("recognizer builds");

    // Two SSN-shaped numbers: one near the keyword, one not.
    let text = "First SSN: 123-45-6789. Unrelated number 987-65-4329 elsewhere.";
    let input = RecognizerInput::new(TextData::new(text.to_owned()));
    let mut entities = recognizer
        .recognize(&input)
        .await
        .expect("recognize")
        .entities;
    assert_eq!(entities.len(), 2, "two SSN matches expected");

    // Snapshot base confidences keyed by match text so we can compare
    // before vs after.
    let mut before: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for e in &entities {
        before.insert(
            text[e.location.start..e.location.end].to_owned(),
            e.confidence.get(),
        );
    }

    let enhancer = ContextEnhancer::builder()
        .with_registry(registry.context_registry())
        .with_default_window(20)
        .with_default_boost(0.3)
        .build()
        .expect("enhancer builds");
    enhancer.enhance(&mut entities, text, &type_map::concurrent::TypeMap::new());

    // First match has `SSN:` within the 20-byte window → boosted.
    let near = entities
        .iter()
        .find(|e| &text[e.location.start..e.location.end] == "123-45-6789")
        .expect("near match present");
    assert!(
        near.confidence.get() > before["123-45-6789"],
        "near-keyword match should be boosted"
    );
    assert!(
        near.trail
            .iter()
            .any(|s| matches!(s.kind, TrailStepKind::Refinement)),
        "near-keyword match should have a Refinement step"
    );
    let TrailProvenance::Pattern(PatternProvenance::Regex { contextual, .. }) =
        &near.trail[0].provenance
    else {
        panic!("expected regex provenance on recognition step");
    };
    assert!(
        *contextual,
        "contextual flag should be set on recognition step"
    );

    // Second match is well outside the 20-byte window → untouched.
    let far = entities
        .iter()
        .find(|e| &text[e.location.start..e.location.end] == "987-65-4329")
        .expect("far match present");
    assert_eq!(
        far.confidence.get(),
        before["987-65-4329"],
        "far-from-keyword match should not be boosted"
    );
    assert!(
        !far.trail
            .iter()
            .any(|s| matches!(s.kind, TrailStepKind::Refinement)),
        "far-from-keyword match should have no Refinement step"
    );
}
