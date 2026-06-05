//! End-to-end: load every shipped pattern + dictionary into one
//! [`PatternRecognizer`], scan each `testdata/inputs/*.txt`, and
//! assert the entities a real document of that category is expected
//! to surface (by substring + kind).
//!
//! These are intentionally substring-based rather than offset-based
//! so the fixtures and shipped regexes can both evolve without
//! brittle byte-position churn.

use nvisy_core::entity::{Entity, EntityKind};
use nvisy_core::modality::Text;
use nvisy_core::{EntityRecognizer, RecognizerInput, TextData};
use nvisy_pattern::{PatternRecognizer, PatternRegistry};

fn shipped_recognizer() -> PatternRecognizer {
    PatternRecognizer::builder()
        .with_registry(PatternRegistry::builtin())
        .build()
        .expect("shipped recognizer builds")
}

async fn scan(text: &str) -> (String, Vec<Entity<Text>>) {
    let recognizer = shipped_recognizer();
    let input = RecognizerInput::new(TextData::new(text.to_owned()));
    let entities = recognizer
        .recognize(&input)
        .await
        .expect("shipped recognize")
        .entities;
    (text.to_owned(), entities)
}

fn assert_match(text: &str, entities: &[Entity<Text>], kind: EntityKind, needle: &str) {
    let hit = entities
        .iter()
        .any(|e| e.entity_kind == kind && &text[e.location.start..e.location.end] == needle);
    assert!(
        hit,
        "expected `{needle}` as {kind:?}; got: {:?}",
        entities
            .iter()
            .map(|e| (e.entity_kind, &text[e.location.start..e.location.end]))
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn contact_inputs_yield_expected_entities() {
    let (text, entities) = scan(include_str!("../testdata/inputs/contact.txt")).await;
    assert_match(
        &text,
        &entities,
        EntityKind::EmailAddress,
        "alice.johnson@example.com",
    );
    assert_match(
        &text,
        &entities,
        EntityKind::Url,
        "https://docs.example.com/proposal",
    );
    assert_match(
        &text,
        &entities,
        EntityKind::Url,
        "http://backup.example.org/proposal-v2",
    );
}

#[tokio::test]
async fn identity_inputs_yield_expected_entities() {
    let (text, entities) = scan(include_str!("../testdata/inputs/identity.txt")).await;
    assert_match(&text, &entities, EntityKind::GovernmentId, "123-45-6789");
    assert_match(&text, &entities, EntityKind::DateOfBirth, "1985-03-14");
}

#[tokio::test]
async fn finance_inputs_yield_expected_entities() {
    let (text, entities) = scan(include_str!("../testdata/inputs/finance.txt")).await;
    assert_match(
        &text,
        &entities,
        EntityKind::PaymentCard,
        "4539 1488 0343 6467",
    );
    assert_match(
        &text,
        &entities,
        EntityKind::CryptoAddress,
        "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
    );
    assert_match(
        &text,
        &entities,
        EntityKind::CryptoAddress,
        "0x742d35Cc6634C0532925a3b844Bc9e7595f6E842",
    );
    // Currency and cryptocurrency dictionaries emit `Currency`;
    // pick up `USD`, `EUR`, `Tether`, `USDC`, …
    assert!(
        entities
            .iter()
            .any(|e| matches!(e.entity_kind, EntityKind::Currency)),
        "expected at least one currency/crypto dictionary hit"
    );
}

#[tokio::test]
async fn credentials_inputs_yield_expected_entities() {
    let (text, entities) = scan(include_str!("../testdata/inputs/credentials.txt")).await;
    assert_match(&text, &entities, EntityKind::ApiKey, "AKIAIOSFODNN7EXAMPLE");
    // Private-key pattern matches the BEGIN header.
    assert!(
        entities
            .iter()
            .any(|e| e.entity_kind == EntityKind::PrivateKey),
        "expected at least one PrivateKey entity"
    );
}

#[tokio::test]
async fn network_inputs_yield_expected_entities() {
    let (text, entities) = scan(include_str!("../testdata/inputs/network.txt")).await;
    assert_match(&text, &entities, EntityKind::IpAddress, "192.168.1.42");
    assert_match(&text, &entities, EntityKind::IpAddress, "10.0.0.7");
    assert_match(&text, &entities, EntityKind::IpAddress, "203.0.113.55");
    assert_match(
        &text,
        &entities,
        EntityKind::IpAddress,
        "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
    );
    assert_match(
        &text,
        &entities,
        EntityKind::MacAddress,
        "00:1A:2B:3C:4D:5E",
    );
}

#[tokio::test]
async fn personal_inputs_yield_expected_entities() {
    let (text, entities) = scan(include_str!("../testdata/inputs/personal.txt")).await;
    assert_match(&text, &entities, EntityKind::DateOfBirth, "04/22/1979");
    assert_match(
        &text,
        &entities,
        EntityKind::DateTime,
        "2024-06-15T09:30:00Z",
    );
    // Nationality and language dictionaries pick up `Italian`,
    // `Canadian`, `English`, `Spanish`.
    assert!(
        entities
            .iter()
            .any(|e| e.entity_kind == EntityKind::Nationality),
        "expected at least one Nationality"
    );
    assert!(
        entities
            .iter()
            .any(|e| e.entity_kind == EntityKind::Language),
        "expected at least one Language"
    );
}
