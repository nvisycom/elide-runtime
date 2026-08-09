//! PCI DSS §3.5.1 — render stored Primary Account Numbers (PAN)
//! unreadable.
//!
//! §3.5.1 lists four acceptable render approaches: one-way hashes
//! based on strong cryptography, truncation, index tokens with
//! securely stored pads, and strong cryptography with associated
//! key-management processes. This module ships two:
//!
//! - [`PciPanRender::Truncate`] → `Truncate { keep_prefix: 6, keep_suffix: 4 }`
//!   — the historical PCI truncation posture. No key material
//!   involved.
//! - [`PciPanRender::HmacSha256`] → `HmacHash { algorithm: Sha256 }`
//!   — the "keyed cryptographic hash" posture PCI DSS v4.0.1
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
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::{Uuid, uuid};

use super::Template;

/// Which PCI DSS §3.5.1-permitted render approach to apply to
/// stored PAN.
///
/// The choice is a real operational decision, not a style knob:
///
/// - [`Truncate`](Self::Truncate) — irreversible, no key material,
///   nothing secret to protect. Destroys uniqueness (two PANs
///   sharing BIN + last-4 collapse to the same string), so it's
///   unsuitable when downstream joins or dedup need per-row
///   identity across a PAN column.
/// - [`HmacSha256`](Self::HmacSha256) — preserves 1:1 uniqueness
///   (same PAN → same digest), enabling joins, dedup, and
///   fraud-scoring on the digest. But now the tenant owns a key
///   the engine reads via [`Engine::with_key_provider`]; a leaked
///   key permits offline PAN enumeration against the shipped
///   digests.
///
/// Callers wanting both dispatched from one policy compose two
/// templates.
///
/// [`Engine::with_key_provider`]: https://docs.rs/nvisy-engine/latest/nvisy_engine/struct.Engine.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PciPanRender {
    /// Truncate stored PAN to the first six (BIN) and last four
    /// digits.
    Truncate,
    /// Replace stored PAN with an HMAC-SHA-256 digest keyed on
    /// the engine's `KeyProvider`.
    HmacSha256,
}

/// The elide-builtin label PCI PAN templates dispatch on.
const PAN_LABEL: LabelRef = LabelRef::from_static("payment_card");

/// PCI DSS §3.5.1 effective date (v4.0.1 mandatory compliance).
const EFFECTIVE_DATE: Date = Date::constant(2025, 3, 31);

const TRUNCATE_POLICY_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000001");
const TRUNCATE_RULE_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000002");
const HMAC_POLICY_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000003");
const HMAC_RULE_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000004");

/// PCI DSS §3.5.1 render template dispatched by `render`.
pub(crate) fn template(render: PciPanRender) -> Template {
    let (id, name, description, policy_id, policy_name, policy_description, rule) = match render {
        PciPanRender::Truncate => (
            "pci_dss_pan_truncate",
            "PCI DSS §3.5.1 PAN — truncate",
            "Render stored PAN unreadable via truncation, keeping the first six \
             (BIN) and last four digits.",
            TRUNCATE_POLICY_ID,
            "pci-dss-pan-truncate",
            "Truncate stored PAN to the first six digits and last four, dropping \
             the middle. Keeps BIN and last-four for downstream lookups without \
             leaving a reversible ciphertext or a key surface to protect.",
            truncate_rule(),
        ),
        PciPanRender::HmacSha256 => (
            "pci_dss_pan_hmac_sha256",
            "PCI DSS §3.5.1 PAN — HMAC-SHA-256",
            "Render stored PAN unreadable via a keyed HMAC-SHA-256 digest. Requires \
             the engine to have a KeyProvider wired.",
            HMAC_POLICY_ID,
            "pci-dss-pan-hmac-sha256",
            "Replace stored PAN with a keyed HMAC-SHA-256 digest. Requires the \
             engine to have a KeyProvider wired via `Engine::with_key_provider`. \
             The key must stay secret; a leaked key permits offline PAN enumeration \
             against the shipped hash.",
            hmac_rule(),
        ),
    };
    Template {
        id: id.into(),
        name: name.into(),
        version: Version::new(1, 0, 0),
        effective_date: EFFECTIVE_DATE,
        description: Some(description.into()),
        policy: PolicyDefinition {
            id: policy_id,
            name: policy_name.into(),
            description: Some(policy_description.to_owned()),
            when: None,
            labels: Labels {
                builtins: vec![PAN_LABEL.clone()],
                custom: Vec::new(),
            },
            groups: Vec::new(),
            rules: vec![rule],
            fallback: None,
            retention: Vec::new(),
        },
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
    fn both_renders_target_payment_card_label() {
        for render in [PciPanRender::Truncate, PciPanRender::HmacSha256] {
            let t = template(render);
            let PolicyRule::Predicated(rule) = &t.policy.rules[0] else {
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
        let trunc_a = template(PciPanRender::Truncate);
        let trunc_b = template(PciPanRender::Truncate);
        assert_eq!(trunc_a.policy.id, trunc_b.policy.id);
        assert_eq!(trunc_a.policy.rules[0].id(), trunc_b.policy.rules[0].id());
        let hmac_a = template(PciPanRender::HmacSha256);
        let hmac_b = template(PciPanRender::HmacSha256);
        assert_eq!(hmac_a.policy.id, hmac_b.policy.id);
        assert_eq!(hmac_a.policy.rules[0].id(), hmac_b.policy.rules[0].id());
        assert_ne!(
            trunc_a.policy.id, hmac_a.policy.id,
            "the two PCI renders must ship distinct policy identities",
        );
    }
}
