//! End-to-end: decode a CSV fixture through the codec registry,
//! detect entities via [`RecognizerRegistryExt::detect`] (which
//! runs Text recognizers against each cell value and lifts every
//! match to a [`TabularLocation`] via [`Handler::lift_chunk`]
//! on the CSV handler), dedup, redact through a [`Tabular`]
//! redaction registry, encode back to bytes, and assert on the
//! result.
//!
//! Compared to the TXT / JSON pipelines, this exercises:
//!
//! 1. Cell-coordinate lifting: each entity's location must point
//!    at the right `(row, col)` plus an intra-cell byte range.
//! 2. Partial-cell redaction: `Mask::stars()` over a payment card
//!    cell rewrites only the card digits, not the surrounding
//!    delimiters or other cells.
//! 3. Anonymizer<Tabular> impls on the built-in [`Replace`] and
//!    [`Mask`] operators newly added to the toolkit.
//!
//! [`TabularLocation`]: nvisy_core::modality::TabularLocation
//! [`Handler::lift_chunk`]: nvisy_codec::Handler::lift_chunk
//! [`RecognizerRegistryExt::detect`]: nvisy_toolkit::detection::RecognizerRegistryExt::detect
//! [`Tabular`]: nvisy_core::modality::Tabular
//! [`Replace`]: nvisy_toolkit::redaction::anonymizer::Replace
//! [`Mask`]: nvisy_toolkit::redaction::anonymizer::Mask

mod common;

use bytes::Bytes;
use nvisy_core::entity::EntityKind;

use crate::common::{
    assert_cell_entity_matched, decode_tabular_buffer, dedup_tabular, detect_per_cell,
    redact_and_encode_tabular, shipped_recognizer, write_redacted_artifact,
};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/pipeline/contact.csv");
const SAMPLE: &str = include_str!("../testdata/pipeline/contact.csv");

#[tokio::test]
async fn csv_pipeline_detects_and_redacts() {
    let recognizer = shipped_recognizer();
    let mut buffer = decode_tabular_buffer(Bytes::from(SAMPLE), "csv")
        .await
        .expect("csv decodes");

    let detected = detect_per_cell(recognizer, &mut buffer)
        .await
        .expect("detect succeeds");

    // Cell coordinates: header row 0 is name,email,phone,card;
    // data rows are 1 (Alice) and 2 (Bob).
    assert_cell_entity_matched(
        "alice.johnson@example.com",
        &detected,
        EntityKind::EmailAddress,
        1,
        1,
        "alice.johnson@example.com",
    );
    assert_cell_entity_matched(
        "bob.smith@example.com",
        &detected,
        EntityKind::EmailAddress,
        2,
        1,
        "bob.smith@example.com",
    );
    assert_cell_entity_matched(
        "+1 (415) 555-0142",
        &detected,
        EntityKind::PhoneNumber,
        1,
        2,
        "+1 (415) 555-0142",
    );
    assert_cell_entity_matched(
        "4111 1111 1111 1111",
        &detected,
        EntityKind::PaymentCard,
        1,
        3,
        "4111 1111 1111 1111",
    );

    let kept = dedup_tabular(detected, &buffer).await;

    let encoded = redact_and_encode_tabular(&mut buffer, &kept)
        .await
        .expect("encode succeeds");

    // Sensitive substrings replaced; replacement tokens present.
    assert!(
        !encoded.contains("alice.johnson@example.com"),
        "alice email survived redaction: {encoded}",
    );
    assert!(
        !encoded.contains("bob.smith@example.com"),
        "bob email survived redaction: {encoded}",
    );
    assert!(
        !encoded.contains("+1 (415) 555-0142"),
        "alice phone survived redaction: {encoded}",
    );
    assert!(
        !encoded.contains("+1 (510) 555-0199"),
        "bob phone survived redaction: {encoded}",
    );
    assert!(
        !encoded.contains("4111 1111 1111 1111"),
        "alice card survived redaction: {encoded}",
    );
    assert!(
        !encoded.contains("5555 5555 5555 4444"),
        "bob card survived redaction: {encoded}",
    );
    assert!(
        encoded.contains("[email_address]"),
        "email replacement token missing: {encoded}",
    );
    assert!(
        encoded.contains("[phone_number]"),
        "phone replacement token missing: {encoded}",
    );

    // CSV structure preserved: header row and non-sensitive cells
    // pass through verbatim.
    assert!(
        encoded.starts_with("name,email,phone,card"),
        "header row lost: {encoded}",
    );
    assert!(
        encoded.contains("Alice Johnson,"),
        "alice name lost: {encoded}",
    );
    assert!(encoded.contains("Bob Smith,"), "bob name lost: {encoded}",);

    write_redacted_artifact(FIXTURE, &encoded);
}
