//! End-to-end HTML codec. The HTML loader chunks the source on
//! text nodes; this test exercises:
//!
//! 1. Entity detection inside per-tag text nodes (PII split across
//!    `<strong>`, `<em>`, `<code>` resolves and feeds the redactor).
//! 2. Tag preservation: `Handler::encode` rebuilds the document
//!    from the parsed tree so `<p>`, `<strong>`, `<em>`, `<code>`,
//!    `<h1>`, etc. survive alongside the rewritten text.
//!
//! Note: entity locations on HTML are text-stream coords (relative
//! to the concatenated text nodes), not raw HTML source byte
//! offsets, so this test asserts by entity-kind presence rather
//! than slicing the source at each entity's range. See
//! `.ignore/html-handler-improvements.md` for the longer-term
//! roadmap (attribute scanning, URL scheme awareness, …).

mod fixtures;

use nvisy_core::entity::EntityKind;

use crate::fixtures::{Fixture, assert_pii_removed, assert_tokens_present};

const FIXTURE: Fixture = Fixture {
    path: concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/contact.html"),
    source: include_str!("../testdata/contact.html"),
    extension: "html",
};

#[tokio::test]
async fn html_codec_detects_and_redacts() {
    let outcome = FIXTURE.run_text_pipeline().await;

    // Detection covered every PII kind in the fixture. Locations
    // index the text-node stream rather than the raw source, so the
    // per-needle slice assertion the txt/json tests use doesn't
    // translate here; presence-by-kind is the right shape.
    for expected in [
        EntityKind::EmailAddress,
        EntityKind::PhoneNumber,
        EntityKind::PaymentCard,
        EntityKind::Iban,
        EntityKind::GovernmentId,
        EntityKind::IpAddress,
    ] {
        assert!(
            outcome.entities.iter().any(|e| e.entity_kind == expected),
            "expected at least one {expected:?} entity; got: {:?}",
            outcome
                .entities
                .iter()
                .map(|e| e.entity_kind)
                .collect::<Vec<_>>()
        );
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

    // HTML structure preserved. The encoder rebuilds the document
    // from the parsed tree, so attribute quoting and whitespace
    // around tags may be normalised — assert presence of the
    // structural elements rather than verbatim byte equality.
    assert!(
        outcome.redacted.contains("<!DOCTYPE html>"),
        "doctype lost: {}",
        outcome.redacted,
    );
    assert!(
        outcome
            .redacted
            .contains("<title>Customer onboarding</title>"),
        "title lost: {}",
        outcome.redacted,
    );
    assert!(
        outcome.redacted.contains("<h1>Customer onboarding</h1>"),
        "h1 lost: {}",
        outcome.redacted,
    );
    assert!(
        outcome.redacted.contains("<strong>"),
        "strong tag lost: {}",
        outcome.redacted,
    );
    assert!(
        outcome.redacted.contains("<em>"),
        "em tag lost: {}",
        outcome.redacted,
    );
    assert!(
        outcome.redacted.contains("<code>"),
        "code tag lost: {}",
        outcome.redacted,
    );
}
