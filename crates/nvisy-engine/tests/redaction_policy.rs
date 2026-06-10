//! End-to-end test for the policy → audit redaction wire shape.
//!
//! Loads each [`TextRedaction`] variant from TOML, asserts the
//! deserialised operator matches the spec, then confirms a full
//! `Action::Redact { operator }` round-trips through the JSON wire
//! format the audit record uses.
//!
//! This exercises the user-visible surface end-to-end without
//! touching the codec handle — the apply-time bridge from operator
//! spec to runnable instance is covered separately by
//! `policy::redaction::Instantiate` impls.

use nvisy_core::modality::Text;
use nvisy_engine::policy::redaction::{HashAlgorithm, TextRedaction};
use nvisy_engine::policy::{Action, Policy, PolicyRule};
use nvisy_toolkit::redaction::AnonymizerId;

/// Each built-in arm deserialises with its declared params.
#[test]
fn text_redaction_each_variant_round_trips() {
    let toml = r#"
        [[arms]]
        kind = "replace"
        template = "[EMAIL]"

        [[arms]]
        kind = "mask"
        mask_char = "*"
        keep_prefix = 4
        keep_suffix = 4

        [[arms]]
        kind = "hash"
        algorithm = "sha256"
        salt = "pepper"

        [[arms]]
        kind = "redact"

        [[arms]]
        kind = "keep"

        [[arms]]
        kind = "custom"
        id = "kms_encrypt"
    "#;

    #[derive(serde::Deserialize)]
    struct Wrapper {
        arms: Vec<TextRedaction>,
    }
    let parsed: Wrapper = toml::from_str(toml).expect("parse arms");
    assert_eq!(parsed.arms.len(), 6);

    match &parsed.arms[0] {
        TextRedaction::Replace { template } => assert_eq!(template, "[EMAIL]"),
        other => panic!("expected Replace, got {other:?}"),
    }
    match &parsed.arms[1] {
        TextRedaction::Mask {
            mask_char,
            keep_prefix,
            keep_suffix,
        } => {
            assert_eq!(*mask_char, '*');
            assert_eq!(*keep_prefix, 4);
            assert_eq!(*keep_suffix, 4);
        }
        other => panic!("expected Mask, got {other:?}"),
    }
    match &parsed.arms[2] {
        TextRedaction::Hash { algorithm, salt } => {
            assert_eq!(*algorithm, HashAlgorithm::Sha256);
            assert_eq!(salt.as_deref(), Some("pepper"));
        }
        other => panic!("expected Hash, got {other:?}"),
    }
    assert!(matches!(parsed.arms[3], TextRedaction::Redact));
    assert!(matches!(parsed.arms[4], TextRedaction::Keep));
    match &parsed.arms[5] {
        TextRedaction::Custom { id } => assert_eq!(id.as_str(), "kms_encrypt"),
        other => panic!("expected Custom, got {other:?}"),
    }
}

/// Defaulted fields (template, mask_char) come back with their
/// declared defaults when the TOML omits them.
#[test]
fn text_redaction_defaults_fill_in() {
    let parsed: TextRedaction = toml::from_str(r#"kind = "replace""#).expect("parse");
    match parsed {
        TextRedaction::Replace { template } => assert_eq!(template, "[{entity_kind}]"),
        other => panic!("expected Replace, got {other:?}"),
    }
    let parsed: TextRedaction = toml::from_str(r#"kind = "mask""#).expect("parse");
    match parsed {
        TextRedaction::Mask {
            mask_char,
            keep_prefix,
            keep_suffix,
        } => {
            assert_eq!(mask_char, '*');
            assert_eq!(keep_prefix, 0);
            assert_eq!(keep_suffix, 0);
        }
        other => panic!("expected Mask, got {other:?}"),
    }
}

/// A full `Action<Text>::Redact { operator }` round-trips through
/// JSON — the wire shape audit records use.
#[test]
fn action_redact_round_trips_through_json() {
    let action: Action<Text> = Action::Redact {
        operator: TextRedaction::Replace {
            template: "[EMAIL]".into(),
        },
    };
    let json = serde_json::to_string(&action).expect("serialize");
    assert!(
        json.contains("\"action\":\"redact\""),
        "expected redact tag, got {json}"
    );
    assert!(
        json.contains("\"kind\":\"replace\""),
        "expected nested replace kind, got {json}"
    );

    let parsed: Action<Text> = serde_json::from_str(&json).expect("deserialize");
    match parsed {
        Action::Redact { operator } => match operator {
            TextRedaction::Replace { template } => assert_eq!(template, "[EMAIL]"),
            other => panic!("expected nested Replace, got {other:?}"),
        },
        other => panic!("expected Redact, got {other:?}"),
    }
}

/// A `Custom`-arm action survives a JSON round-trip with its
/// [`AnonymizerId<Text>`] intact.
#[test]
fn action_redact_custom_id_round_trips() {
    let id = AnonymizerId::<Text>::from_static("kms_encrypt");
    let action: Action<Text> = Action::Redact {
        operator: TextRedaction::Custom { id: id.clone() },
    };
    let json = serde_json::to_string(&action).expect("serialize");
    let parsed: Action<Text> = serde_json::from_str(&json).expect("deserialize");
    match parsed {
        Action::Redact {
            operator: TextRedaction::Custom { id: round_tripped },
        } => assert_eq!(round_tripped, id),
        other => panic!("expected Custom redact, got {other:?}"),
    }
}

/// A `Policy<Text>` with mixed rules deserialises from TOML and the
/// `default_action` arm carries its operator spec verbatim.
#[test]
fn policy_with_redact_rules_round_trips_from_toml() {
    let toml = r##"
        id = "00000000-0000-0000-0000-000000000000"
        name = "email-and-card"
        version = "1.0.0"
        description = "redact emails, mask cards, drop everything else by default"

        [[rules]]
        action = "redact"
        operator = { kind = "replace", template = "[EMAIL]" }

        [rules.selector]
        entityKinds = ["email_address"]

        [[rules]]
        action = "redact"
        operator = { kind = "mask", mask_char = "#", keep_suffix = 4 }

        [rules.selector]
        entityKinds = ["payment_card"]

        [defaultAction]
        action = "redact"
        operator = { kind = "redact" }
    "##;

    let policy: Policy<Text> = toml::from_str(toml).expect("parse policy");
    assert_eq!(policy.name, "email-and-card");
    assert_eq!(policy.rules.len(), 2);

    // First rule: email → Replace { "[EMAIL]" }.
    let PolicyRule { action, .. } = &policy.rules[0];
    match action {
        Action::Redact {
            operator: TextRedaction::Replace { template },
        } => assert_eq!(template, "[EMAIL]"),
        other => panic!("expected Replace, got {other:?}"),
    }

    // Second rule: card → Mask { '#', keep last 4 }.
    let PolicyRule { action, .. } = &policy.rules[1];
    match action {
        Action::Redact {
            operator:
                TextRedaction::Mask {
                    mask_char,
                    keep_suffix,
                    ..
                },
        } => {
            assert_eq!(*mask_char, '#');
            assert_eq!(*keep_suffix, 4);
        }
        other => panic!("expected Mask, got {other:?}"),
    }

    // Default action: full Redact (drop the span entirely).
    match policy.default_action.as_ref() {
        Some(Action::Redact {
            operator: TextRedaction::Redact,
        }) => {}
        other => panic!("expected default Redact, got {other:?}"),
    }
}
