//! End-to-end JSON codec. Exercises the JSON-specific guarantees
//! on top of the shared text pipeline:
//!
//! 1. Lifted offsets index the raw JSON source — the quoted form
//!    plus its escape table — so redaction edits the right bytes.
//! 2. `Handler::encode` preserves the original indentation, key
//!    order, and whitespace verbatim for every slot the redaction
//!    didn't touch (the slot model in `json_handler`).

mod fixtures;

use nvisy_core::entity::EntityKind;

use crate::fixtures::{Fixture, assert_pii_removed, assert_text_entity, assert_tokens_present};

const FIXTURE: Fixture = Fixture {
    path: concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/contact.json"),
    source: include_str!("../testdata/contact.json"),
    extension: "json",
};

#[tokio::test]
async fn json_codec_detects_and_redacts() {
    let outcome = FIXTURE.run_text_pipeline().await;

    // Lifted offsets index the raw JSON source — quoted form plus
    // its escape table — so slicing the fixture by an entity's
    // location yields the same bytes the recognizer matched.
    for (kind, needle) in [
        (EntityKind::EmailAddress, "alice.johnson@example.com"),
        (EntityKind::EmailAddress, "bob.smith@example.com"),
        (EntityKind::PhoneNumber, "+1 (415) 555-0142"),
        (EntityKind::PhoneNumber, "+1 (510) 555-0199"),
        (EntityKind::PaymentCard, "4111 1111 1111 1111"),
        (EntityKind::PaymentCard, "5555 5555 5555 4444"),
        (EntityKind::Iban, "GB29 NWBK 6016 1331 9268 19"),
        (EntityKind::Iban, "DE89 3704 0044 0532 0130 00"),
        (EntityKind::GovernmentId, "123-45-6789"),
        (EntityKind::GovernmentId, "234-56-7890"),
        (EntityKind::IpAddress, "192.168.1.42"),
        (EntityKind::IpAddress, "10.0.0.7"),
    ] {
        assert_text_entity(FIXTURE.source, &outcome.entities, kind, needle);
    }

    assert_pii_removed(
        &outcome.redacted,
        &[
            "alice.johnson@example.com",
            "bob.smith@example.com",
            "+1 (415) 555-0142",
            "+1 (510) 555-0199",
            "4111 1111 1111 1111",
            "5555 5555 5555 4444",
            "GB29 NWBK 6016 1331 9268 19",
            "DE89 3704 0044 0532 0130 00",
            "123-45-6789",
            "234-56-7890",
            "192.168.1.42",
            "10.0.0.7",
        ],
    );
    assert_tokens_present(
        &outcome.redacted,
        &[
            "[email_address]",
            "[phone_number]",
            "[iban]",
            "[government_id]",
            "[ip_address]",
        ],
    );

    // JSON structure preserved: the slot model encodes every
    // untouched span byte-for-byte, so indentation, key order, and
    // surrounding punctuation survive intact.
    assert!(
        outcome
            .redacted
            .contains("\"subject\": \"Customer onboarding\""),
        "subject line lost: {}",
        outcome.redacted,
    );
    assert!(
        outcome.redacted.contains("\"contacts\":"),
        "contacts key lost: {}",
        outcome.redacted,
    );
    assert!(
        outcome.redacted.contains("\"name\": \"Alice Johnson\""),
        "alice name lost: {}",
        outcome.redacted,
    );
    assert!(
        outcome.redacted.contains("\"name\": \"Bob Smith\""),
        "bob name lost: {}",
        outcome.redacted,
    );
    assert!(
        outcome.redacted.ends_with("\n"),
        "trailing newline lost: {:?}",
        outcome.redacted,
    );

    // The output is still well-formed JSON.
    serde_json::from_str::<serde_json::Value>(&outcome.redacted)
        .expect("redacted output is valid JSON");
}
