//! PCI DSS §3.5.1 — render stored Primary Account Numbers (PAN)
//! unreadable.
//!
//! §3.5.1 lists four acceptable render approaches: one-way hashes
//! based on strong cryptography, truncation, index tokens with
//! securely stored pads, and strong cryptography with associated
//! key-management processes. This module ships the two the runtime
//! covers today:
//!
//! - [`truncate_template`] → `Truncate { keep_prefix: 6, keep_suffix: 4 }`
//!   — the historical PCI truncation posture. No key material
//!   involved.
//! - [`hmac_template`] → `HmacHash { algorithm: Sha256 }` — the
//!   "keyed cryptographic hash" posture PCI DSS v4.0.1
//!   introduces. Requires the engine to have a `KeyProvider`
//!   wired.
//!
//! Both target the elide-builtin `payment_card` label. No
//! [`LabelGroup`] — one label, one rule per template. Callers
//! wanting both dispatched from one policy compose the two
//! [`PolicyDefinition`]s themselves.
//!
//! [`LabelGroup`]: nvisy_policy::LabelGroup
//! [`PolicyDefinition`]: nvisy_policy::PolicyDefinition

use elide_core::entity::LabelRef;
use jiff::civil::Date;
use nvisy_policy::predicate::Predicate;
use nvisy_policy::redaction::{ModalityRedactions, Sha2Algorithm, TextRedaction};
use nvisy_policy::{Labels, PolicyDefinition, PolicyRule, PredicatedRule};
use semver::Version;
use uuid::{Uuid, uuid};

use super::Template;

/// The elide-builtin label PCI PAN templates dispatch on.
const PAN_LABEL: LabelRef = LabelRef::from_static("payment_card");

/// PCI DSS §3.5.1 effective date (v4.0.1 mandatory compliance).
const EFFECTIVE_DATE: Date = Date::constant(2025, 3, 31);

const TRUNCATE_POLICY_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000001");
const TRUNCATE_RULE_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000002");
const HMAC_POLICY_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000003");
const HMAC_RULE_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000004");

/// PCI DSS §3.5.1 — truncation posture: keep the first six and
/// last four digits, drop the middle.
pub(crate) fn truncate_template() -> Template {
    Template {
        id: "pci_dss_pan_truncate".into(),
        name: "PCI DSS §3.5.1 PAN — truncate".into(),
        version: Version::new(1, 0, 0),
        effective_date: EFFECTIVE_DATE,
        description: Some(
            "Render stored PAN unreadable via truncation, keeping the first six \
             (BIN) and last four digits."
                .into(),
        ),
        policies: vec![PolicyDefinition {
            id: TRUNCATE_POLICY_ID,
            name: "pci-dss-pan-truncate".into(),
            description: Some(
                "Truncate stored PAN to the first six digits and last four, dropping \
                 the middle. Keeps BIN and last-four for downstream lookups without \
                 leaving a reversible ciphertext or a key surface to protect."
                    .to_owned(),
            ),
            when: None,
            labels: Labels {
                builtins: vec![PAN_LABEL.clone()],
                custom: Vec::new(),
            },
            groups: Vec::new(),
            rules: vec![truncate_rule()],
            fallback: None,
            retention: Vec::new(),
        }],
    }
}

/// PCI DSS §3.5.1 — keyed cryptographic hash posture: HMAC-SHA-256
/// with a per-tenant key.
pub(crate) fn hmac_template() -> Template {
    Template {
        id: "pci_dss_pan_hmac".into(),
        name: "PCI DSS §3.5.1 PAN — HMAC-SHA-256".into(),
        version: Version::new(1, 0, 0),
        effective_date: EFFECTIVE_DATE,
        description: Some(
            "Render stored PAN unreadable via a keyed HMAC-SHA-256 digest. Requires \
             the engine to have a KeyProvider wired."
                .into(),
        ),
        policies: vec![PolicyDefinition {
            id: HMAC_POLICY_ID,
            name: "pci-dss-pan-hmac".into(),
            description: Some(
                "Replace stored PAN with a keyed HMAC-SHA-256 digest. Requires the \
                 engine to have a KeyProvider wired via `Engine::with_key_provider`. \
                 The key must stay secret; a leaked key permits offline PAN enumeration \
                 against the shipped hash."
                    .to_owned(),
            ),
            when: None,
            labels: Labels {
                builtins: vec![PAN_LABEL.clone()],
                custom: Vec::new(),
            },
            groups: Vec::new(),
            rules: vec![hmac_rule()],
            fallback: None,
            retention: Vec::new(),
        }],
    }
}

fn truncate_rule() -> PolicyRule {
    PolicyRule::Predicated(Box::new(PredicatedRule {
        id: TRUNCATE_RULE_ID,
        name: "pci-truncate-pan".into(),
        description: Some(
            "Drop the middle of every payment_card value, keeping the first six \
             (BIN) and last four digits."
                .to_owned(),
        ),
        predicate: single_label(PAN_LABEL.clone()),
        action: ModalityRedactions::text(TextRedaction::Truncate {
            keep_prefix: 6,
            keep_suffix: 4,
        }),
    }))
}

fn hmac_rule() -> PolicyRule {
    PolicyRule::Predicated(Box::new(PredicatedRule {
        id: HMAC_RULE_ID,
        name: "pci-hmac-pan".into(),
        description: Some(
            "Replace every payment_card value with an HMAC-SHA-256 digest keyed on \
             the engine's KeyProvider."
                .to_owned(),
        ),
        predicate: single_label(PAN_LABEL.clone()),
        action: ModalityRedactions::text(TextRedaction::HmacHash {
            algorithm: Sha2Algorithm::Sha256,
        }),
    }))
}

fn single_label(label: LabelRef) -> Predicate {
    Predicate::LabelOneOf {
        labels: vec![label],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_rule_keeps_bin_and_last_four() {
        let PolicyRule::Predicated(rule) = &truncate_template().policies[0].rules[0] else {
            panic!("expected Predicated rule");
        };
        assert!(matches!(
            rule.action.text,
            Some(TextRedaction::Truncate {
                keep_prefix: 6,
                keep_suffix: 4,
            })
        ));
    }

    #[test]
    fn hmac_rule_uses_sha256_by_default() {
        let PolicyRule::Predicated(rule) = &hmac_template().policies[0].rules[0] else {
            panic!("expected Predicated rule");
        };
        assert!(matches!(
            rule.action.text,
            Some(TextRedaction::HmacHash {
                algorithm: Sha2Algorithm::Sha256,
            })
        ));
    }

    #[test]
    fn both_templates_target_payment_card_label() {
        for template in [truncate_template(), hmac_template()] {
            let PolicyRule::Predicated(rule) = &template.policies[0].rules[0] else {
                panic!("expected Predicated rule");
            };
            let Predicate::LabelOneOf { labels } = &rule.predicate else {
                panic!("expected LabelOneOf predicate");
            };
            assert_eq!(labels.len(), 1);
            assert_eq!(labels[0], PAN_LABEL);
        }
    }

    #[test]
    fn template_ids_are_stable_across_calls() {
        let trunc_a = truncate_template();
        let trunc_b = truncate_template();
        assert_eq!(trunc_a.policies[0].id, trunc_b.policies[0].id);
        assert_eq!(
            trunc_a.policies[0].rules[0].id(),
            trunc_b.policies[0].rules[0].id(),
        );
        let hmac_a = hmac_template();
        let hmac_b = hmac_template();
        assert_eq!(hmac_a.policies[0].id, hmac_b.policies[0].id);
        assert_eq!(
            hmac_a.policies[0].rules[0].id(),
            hmac_b.policies[0].rules[0].id(),
        );
        assert_ne!(
            trunc_a.policies[0].id, hmac_a.policies[0].id,
            "the two PCI templates must ship distinct policy identities",
        );
    }
}
