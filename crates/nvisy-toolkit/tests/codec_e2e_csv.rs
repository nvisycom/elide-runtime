//! End-to-end CSV codec. On top of the shared tabular pipeline
//! shape (`decode → detect-per-cell → dedup → redact + encode`),
//! this test exercises:
//!
//! 1. Cell-coordinate lifting: each entity's location must point at
//!    the right `(row, col)` plus an intra-cell byte range
//!    (`Handler::lift_chunk` on the CSV handler).
//! 2. Partial-cell redaction: `Mask::stars()` over a payment card
//!    cell rewrites only the card digits, not surrounding bytes.
//! 3. `Anonymizer<Tabular>` impls on built-in [`Replace`] and
//!    [`Mask`] operators.
//!
//! [`Replace`]: nvisy_toolkit::redaction::anonymizer::Replace
//! [`Mask`]: nvisy_toolkit::redaction::anonymizer::Mask

mod fixtures;

use nvisy_core::entity::builtins;

use crate::fixtures::{Fixture, assert_pii_removed, assert_tabular_entity, assert_tokens_present};

const FIXTURE: Fixture = Fixture {
    path: concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/contact.csv"),
    source: include_str!("../testdata/contact.csv"),
    extension: "csv",
};

#[tokio::test]
async fn csv_codec_detects_and_redacts() {
    let outcome = FIXTURE.run_tabular_pipeline().await;

    // Header row 0 is name,email,phone,card,iban,ssn,host;
    // data rows are 1 (Alice) and 2 (Bob).
    for (kind, row, col, cell) in [
        (
            builtins::EMAIL_ADDRESS.label_ref(),
            1,
            1,
            "alice.johnson@example.com",
        ),
        (
            builtins::EMAIL_ADDRESS.label_ref(),
            2,
            1,
            "bob.smith@example.com",
        ),
        (
            builtins::PHONE_NUMBER.label_ref(),
            1,
            2,
            "+1 (415) 555-0142",
        ),
        (
            builtins::PHONE_NUMBER.label_ref(),
            2,
            2,
            "+1 (510) 555-0199",
        ),
        (
            builtins::PAYMENT_CARD.label_ref(),
            1,
            3,
            "4111 1111 1111 1111",
        ),
        (
            builtins::PAYMENT_CARD.label_ref(),
            2,
            3,
            "5555 5555 5555 4444",
        ),
        (
            builtins::IBAN.label_ref(),
            1,
            4,
            "GB29 NWBK 6016 1331 9268 19",
        ),
        (
            builtins::IBAN.label_ref(),
            2,
            4,
            "DE89 3704 0044 0532 0130 00",
        ),
        (builtins::GOVERNMENT_ID.label_ref(), 1, 5, "123-45-6789"),
        (builtins::GOVERNMENT_ID.label_ref(), 2, 5, "234-56-7890"),
        (builtins::IP_ADDRESS.label_ref(), 1, 6, "192.168.1.42"),
        (builtins::IP_ADDRESS.label_ref(), 2, 6, "10.0.0.7"),
    ] {
        assert_tabular_entity(cell, &outcome.entities, kind, row, col, cell);
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

    // CSV structure preserved: header row and non-sensitive cells
    // pass through verbatim.
    assert!(
        outcome
            .redacted
            .starts_with("name,email,phone,card,iban,ssn,host"),
        "header row lost: {}",
        outcome.redacted,
    );
    assert!(
        outcome.redacted.contains("Alice Johnson,"),
        "alice name lost: {}",
        outcome.redacted,
    );
    assert!(
        outcome.redacted.contains("Bob Smith,"),
        "bob name lost: {}",
        outcome.redacted,
    );
}
