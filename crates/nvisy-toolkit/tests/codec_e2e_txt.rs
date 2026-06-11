//! End-to-end TXT codec: decode → detect → dedup → redact + encode.
//!
//! `Handler::encode` is the canonical reassembly path — the codec
//! handles line terminators so the test doesn't have to.

mod fixtures;

use nvisy_core::entity::EntityKind;

use crate::fixtures::{Fixture, assert_pii_removed, assert_text_entity, assert_tokens_present};

const FIXTURE: Fixture = Fixture {
    path: concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/contact.txt"),
    source: include_str!("../testdata/contact.txt"),
    extension: "txt",
};

#[tokio::test]
async fn txt_codec_detects_and_redacts() {
    let outcome = FIXTURE.run_text_pipeline().await;

    for (kind, needle) in [
        (EntityKind::EmailAddress, "alice.johnson@example.com"),
        (EntityKind::PhoneNumber, "+1 (415) 555-0142"),
        (EntityKind::PaymentCard, "4111 1111 1111 1111"),
        (EntityKind::Iban, "GB29 NWBK 6016 1331 9268 19"),
        (EntityKind::GovernmentId, "123-45-6789"),
        (EntityKind::IpAddress, "192.168.1.42"),
    ] {
        assert_text_entity(FIXTURE.source, &outcome.entities, kind, needle);
    }

    assert_pii_removed(
        &outcome.redacted,
        &[
            "alice.johnson@example.com",
            "+1 (415) 555-0142",
            "4111 1111 1111 1111",
            "GB29 NWBK 6016 1331 9268 19",
            "123-45-6789",
            "192.168.1.42",
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

    // Non-sensitive structure preserved by the txt codec's encode.
    assert!(
        outcome.redacted.starts_with("Subject: Customer onboarding"),
        "subject line lost: {}",
        outcome.redacted,
    );
    assert!(
        outcome.redacted.contains("Best,\nBob"),
        "signature lost: {}",
        outcome.redacted,
    );
}
