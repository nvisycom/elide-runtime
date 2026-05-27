//! Per-category input-fixture detection tests.
//!
//! Each fixture in `testdata/inputs/` is a short, realistic passage
//! containing entities a real document of that category would carry.
//! Each test asserts the expected [`EntityKind`]s all appear in the
//! engine's output, doubling as documentation of what the built-in
//! patterns detect for that category.

use std::collections::HashSet;
use std::path::PathBuf;

use nvisy_ontology::entity::EntityKind;
use nvisy_pattern::PatternEngine;
use nvisy_pattern::filter::PatternContext;

fn fixture(name: &str) -> String {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "testdata",
        "inputs",
        &format!("{name}.txt"),
    ]
    .iter()
    .collect();
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()))
}

fn detected_kinds(text: &str) -> HashSet<EntityKind> {
    let engine = PatternEngine::instance();
    engine
        .scan_text(text, &PatternContext::default())
        .into_iter()
        .map(|e| e.entity_kind)
        .collect()
}

fn assert_detects(text: &str, expected: &[EntityKind]) {
    let kinds = detected_kinds(text);
    let missing: Vec<_> = expected.iter().filter(|k| !kinds.contains(k)).collect();
    assert!(
        missing.is_empty(),
        "missing expected entity kinds {missing:?}; got {kinds:?}",
    );
}

#[test]
fn contact_fixture_detects_email_phone_url() {
    assert_detects(
        &fixture("contact"),
        &[
            EntityKind::EmailAddress,
            EntityKind::PhoneNumber,
            EntityKind::Url,
        ],
    );
}

#[test]
fn credentials_fixture_detects_keys_and_tokens() {
    assert_detects(
        &fixture("credentials"),
        &[
            EntityKind::ApiKey,
            EntityKind::AuthToken,
            EntityKind::PrivateKey,
        ],
    );
}

#[test]
fn finance_fixture_detects_payment_and_crypto_entities() {
    assert_detects(
        &fixture("finance"),
        &[
            EntityKind::Iban,
            EntityKind::SwiftCode,
            EntityKind::BankRouting,
            EntityKind::PaymentCard,
            EntityKind::CryptoAddress,
            EntityKind::Amount,
        ],
    );
}

#[test]
fn identity_fixture_detects_government_ids() {
    assert_detects(
        &fixture("identity"),
        &[
            EntityKind::GovernmentId,
            EntityKind::DriversLicense,
            EntityKind::PassportNumber,
            EntityKind::PostalCode,
            EntityKind::DateOfBirth,
        ],
    );
}

#[test]
fn network_fixture_detects_ip_and_mac_addresses() {
    assert_detects(
        &fixture("network"),
        &[EntityKind::IpAddress, EntityKind::MacAddress],
    );
}

#[test]
fn personal_fixture_detects_demographics_and_dates() {
    assert_detects(
        &fixture("personal"),
        &[
            EntityKind::DateOfBirth,
            EntityKind::DateTime,
            EntityKind::Nationality,
            EntityKind::Language,
            EntityKind::Religion,
        ],
    );
}
