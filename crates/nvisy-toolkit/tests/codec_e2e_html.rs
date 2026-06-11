//! End-to-end HTML codec. The HTML loader chunks the source on a
//! heterogeneous redactable-item stream — text nodes, comments,
//! scanned attributes, and URL bodies inside scheme-known
//! attributes (`mailto:`, `tel:`, `sms:`, `callto:`). This test
//! exercises:
//!
//! 1. Detection inside per-tag text nodes.
//! 2. Detection inside the default-scanned attributes (`alt`,
//!    `title`, `value`, `placeholder`, `aria-label`,
//!    `aria-describedby`, `data-*`).
//! 3. Detection inside HTML comments (scanned by default).
//! 4. Detection inside `mailto:` and `tel:` URL bodies, with the
//!    scheme prefix and any trailing query suffix re-attached
//!    after redact.
//! 5. Tag preservation: `Handler::encode` rebuilds the document
//!    from the parsed tree so structural elements survive
//!    alongside the rewritten content.
//!
//! Note: entity locations on HTML are item-stream coords (relative
//! to the concatenated text + attribute + comment + URL-body
//! stream), not raw HTML source byte offsets, so this test asserts
//! by entity-kind presence rather than slicing the source at each
//! entity's range. See `.ignore/html-handler-improvements.md`.

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

    // Cover every PII string in the fixture — text-node, attribute,
    // comment, and URL-body locations. If any of these survive,
    // it's a regression somewhere in the redactable-item pipeline.
    assert_pii_removed(
        &outcome.redacted,
        &[
            // Text-node values.
            "alice.johnson@example.com",
            "+1 (415) 555-0142",
            "4111 1111 1111 1111",
            "GB29 NWBK 6016 1331 9268 19",
            "123-45-6789",
            "192.168.1.42",
            // Attribute values (alt, aria-label, placeholder, data-*).
            "Avatar for alice.johnson@example.com",
            "Call Alice at +1 (415) 555-0142",
            "Backup contact e.g. carol.lee@example.com",
            // Comment body.
            "bob.smith@example.com",
            // URL bodies (mailto: + tel:).
            "mailto:alice.johnson@example.com",
            "tel:+14155550142",
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
    // URL scheme prefixes survive: redact only edits the body and
    // re-attaches `scheme:` plus any trailing `?…` / `#…` suffix.
    assert!(
        outcome.redacted.contains("href=\"mailto:"),
        "mailto: scheme prefix lost: {}",
        outcome.redacted,
    );
    assert!(
        outcome.redacted.contains("?subject=Welcome"),
        "mailto: query suffix lost: {}",
        outcome.redacted,
    );
    assert!(
        outcome.redacted.contains("href=\"tel:"),
        "tel: scheme prefix lost: {}",
        outcome.redacted,
    );
    // Comments survive as `<!-- … -->` after their bodies are
    // rewritten.
    assert!(
        outcome.redacted.contains("<!--"),
        "comment delimiter lost: {}",
        outcome.redacted,
    );
}
