//! End-to-end: decode a JSON fixture through the codec registry,
//! detect entities via `RecognizerRegistryExt::detect` (which
//! lifts each match from chunk-local value coordinates to JSON
//! source-byte coordinates via `Handler::lift_chunk`),
//! dedup, redact through the shipped redaction registry, encode
//! back to bytes, and assert on the result.
//!
//! Compared to the TXT pipeline this exercises two JSON-specific
//! guarantees:
//!
//! 1. Lifted offsets index the raw JSON source — the quoted form
//!    plus its escape table — so redaction edits the right bytes.
//! 2. `Handler::encode` preserves the original indentation, key
//!    order, and whitespace verbatim for every slot the redaction
//!    didn't touch (the slot model in `json_handler`).

mod common;

use bytes::Bytes;
use nvisy_core::entity::EntityKind;

use crate::common::{
    assert_entity_matched, decode_text_buffer, dedup, detect_per_chunk, redact_and_encode,
    shipped_recognizer, write_redacted_artifact,
};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/testdata/pipeline/contact.json"
);
const SAMPLE: &str = include_str!("../testdata/pipeline/contact.json");

#[tokio::test]
async fn json_pipeline_detects_and_redacts() {
    let recognizer = shipped_recognizer();
    let mut buffer = decode_text_buffer(Bytes::from(SAMPLE), "json")
        .await
        .expect("json decodes");

    let detected = detect_per_chunk(recognizer, &mut buffer)
        .await
        .expect("detect succeeds");

    // Lifted offsets index the raw JSON source — quoted form plus
    // its escape table — so slicing `SAMPLE` by an entity's
    // location yields the same bytes the recognizer matched.
    assert_entity_matched(
        SAMPLE,
        &detected,
        EntityKind::EmailAddress,
        "alice.johnson@example.com",
    );
    assert_entity_matched(
        SAMPLE,
        &detected,
        EntityKind::EmailAddress,
        "bob.smith@example.com",
    );
    assert_entity_matched(
        SAMPLE,
        &detected,
        EntityKind::PhoneNumber,
        "+1 (415) 555-0142",
    );
    assert_entity_matched(
        SAMPLE,
        &detected,
        EntityKind::PaymentCard,
        "4111 1111 1111 1111",
    );

    let kept = dedup(detected, &buffer).await;

    let encoded = redact_and_encode(&mut buffer, &kept)
        .await
        .expect("encode succeeds");

    // Sensitive substrings replaced; replacement tokens present.
    assert!(
        !encoded.contains("alice.johnson@example.com"),
        "email survived redaction: {encoded}",
    );
    assert!(
        !encoded.contains("bob.smith@example.com"),
        "second email survived redaction: {encoded}",
    );
    assert!(
        !encoded.contains("+1 (415) 555-0142"),
        "phone survived redaction: {encoded}",
    );
    assert!(
        !encoded.contains("4111 1111 1111 1111"),
        "raw card survived redaction: {encoded}",
    );
    assert!(
        encoded.contains("[email_address]"),
        "email replacement token missing: {encoded}",
    );
    assert!(
        encoded.contains("[phone_number]"),
        "phone replacement token missing: {encoded}",
    );

    // JSON structure preserved: the slot model encodes every
    // untouched span byte-for-byte, so indentation, key order, and
    // surrounding punctuation survive intact.
    assert!(
        encoded.contains("\"subject\": \"Customer onboarding\""),
        "subject line lost: {encoded}",
    );
    assert!(
        encoded.contains("\"contacts\":"),
        "contacts key lost: {encoded}",
    );
    assert!(
        encoded.contains("\"name\": \"Alice Johnson\""),
        "alice name lost: {encoded}",
    );
    assert!(
        encoded.contains("\"name\": \"Bob Smith\""),
        "bob name lost: {encoded}",
    );
    assert!(
        encoded.ends_with("\n"),
        "trailing newline lost: {encoded:?}",
    );

    // The output is still well-formed JSON.
    serde_json::from_str::<serde_json::Value>(&encoded).expect("redacted output is valid JSON");

    write_redacted_artifact(FIXTURE, &encoded);
}
