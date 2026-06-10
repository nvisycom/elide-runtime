//! End-to-end: decode a `.txt` fixture through the codec registry,
//! detect entities chunk-by-chunk, dedup with the standard
//! confidence threshold, redact through the shipped redaction
//! registry, encode back through the codec, and assert on the
//! result.
//!
//! `Handler::encode` is the canonical reassembly path — the codec
//! handles line terminators so the test doesn't have to.

mod common;

use bytes::Bytes;
use nvisy_core::entity::EntityKind;

use crate::common::{
    assert_entity_matched, decode_text_buffer, dedup, detect_per_chunk, redact_and_encode,
    shipped_recognizer, write_redacted_artifact,
};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/pipeline/contact.txt");
const SAMPLE: &str = include_str!("../testdata/pipeline/contact.txt");

#[tokio::test]
async fn txt_pipeline_detects_and_redacts() {
    let recognizer = shipped_recognizer();
    let mut buffer = decode_text_buffer(Bytes::from(SAMPLE), "txt")
        .await
        .expect("txt decodes");

    let detected = detect_per_chunk(recognizer, &mut buffer)
        .await
        .expect("detect succeeds");

    assert_entity_matched(
        SAMPLE,
        &detected,
        EntityKind::EmailAddress,
        "alice.johnson@example.com",
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

    // The fixture decodes into multiple lines; dedup uses the
    // buffer as its TextAt/DataAt resolver.
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

    // Non-sensitive structure preserved by the txt codec's encode.
    assert!(
        encoded.starts_with("Subject: Customer onboarding"),
        "subject line lost: {encoded}",
    );
    assert!(encoded.contains("Best,\nBob"), "signature lost: {encoded}",);

    write_redacted_artifact(FIXTURE, &encoded);
}
