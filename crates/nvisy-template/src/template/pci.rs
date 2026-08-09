//! PCI DSS — Primary Account Number (PAN) and Sensitive
//! Authentication Data (SAV) postures.
//!
//! Two families ship from this module:
//!
//! ## §3.5.1 — render stored PAN unreadable
//!
//! §3.5.1 lists four acceptable render approaches: one-way hashes
//! based on strong cryptography, truncation, index tokens with
//! securely stored pads, and strong cryptography with associated
//! key-management processes. This module ships four render
//! variants covering (a) and (b):
//!
//! - [`PciPanRender::Truncate`] → `Truncate { keep_prefix: 6, keep_suffix: 4 }`
//!   — the historical PCI truncation posture. Keeps BIN and
//!   last-four for downstream lookups. No key material involved.
//! - [`PciPanRender::TruncateLastFour`] → `Truncate { keep_prefix: 0, keep_suffix: 4 }`
//!   — the stricter §3.5.1.1 posture (mandatory 2025-03-31): safe
//!   when a downstream system also stores a hashed version of the
//!   same PAN. Loses the BIN.
//! - [`PciPanRender::HmacSha256`] → `HmacHash { algorithm: Sha256 }`
//!   — the "keyed cryptographic hash" posture PCI DSS v4.0.1
//!   introduces. Requires the engine to have a `KeyProvider`
//!   wired.
//! - [`PciPanRender::HmacSha512`] → `HmacHash { algorithm: Sha512 }`
//!   — same posture as `HmacSha256` with SHA-512 (§A2.1 allows
//!   SHA-256 or better).
//!
//! All render variants target the elide-builtin `payment_card`
//! label. No [`LabelGroup`] — one label, one rule per template.
//! Callers wanting more than one dispatched from one policy
//! compose the [`PolicyDefinition`]s themselves.
//!
//! ## §3.3.1 — never store Sensitive Authentication Data
//!
//! §3.3.1 prohibits storage of SAV after authorization completes
//! (CVV/CVC, track data, PIN blocks). Unlike PAN, the correct
//! posture is not "render unreadable" but *erase* — SAV is never
//! allowed to persist. [`sav_template`] ships a single-posture
//! template targeting every elide-builtin SAV label
//! (`card_security_code`, `card_track_data`, `pin_block`) with
//! plain [`Erase`].
//!
//! [`Erase`]: nvisy_policy::redaction::TextRedaction::Erase
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

/// Which PCI DSS subsection this template addresses.
///
/// - [`PanRender`](Self::PanRender) — §3.5.1 render posture for
///   stored Primary Account Numbers. Carries a [`PciPanRender`]
///   picking between the shipped render approaches.
/// - [`SavErase`](Self::SavErase) — §3.3.1 prohibition on
///   storing Sensitive Authentication Data (CVV/CVC, track data,
///   PIN blocks) after authorization. No options — SAV has one
///   posture: erase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "part", rename_all = "snake_case")]
pub enum PciDssPart {
    /// §3.5.1 PAN render posture.
    PanRender {
        /// Which of the §3.5.1-permitted render approaches to
        /// apply.
        render: PciPanRender,
    },
    /// §3.3.1 Sensitive Authentication Data erasure.
    SavErase,
}

/// Which PCI DSS §3.5.1-permitted render approach to apply to
/// stored PAN.
///
/// The truncation / hash split is a real operational decision,
/// not a style knob:
///
/// - Truncation is irreversible with no key material to protect;
///   destroys uniqueness (two PANs sharing the retained digits
///   collapse to the same string), so unsuitable when downstream
///   joins or dedup need per-row identity across a PAN column.
/// - HMAC preserves 1:1 uniqueness (same PAN → same digest),
///   enabling joins, dedup, and fraud-scoring on the digest. But
///   the tenant owns a key the engine reads via
///   [`Engine::with_key_provider`]; a leaked key permits offline
///   PAN enumeration against the shipped digests.
///
/// The two axes inside truncation and inside HMAC are narrower.
///
/// Callers wanting more than one dispatched from one policy
/// compose multiple templates.
///
/// [`Engine::with_key_provider`]: https://docs.rs/nvisy-engine/latest/nvisy_engine/struct.Engine.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PciPanRender {
    /// Truncate stored PAN to the first six (BIN) and last four
    /// digits.
    Truncate,
    /// Truncate stored PAN to the last four digits only (BIN
    /// dropped). The §3.5.1.1 posture required when a downstream
    /// system stores a hashed version of the same PAN — see
    /// PCI DSS v4.0.1 §3.5.1.1.
    TruncateLastFour,
    /// Replace stored PAN with an HMAC-SHA-256 digest keyed on
    /// the engine's `KeyProvider`.
    HmacSha256,
    /// Replace stored PAN with an HMAC-SHA-512 digest keyed on
    /// the engine's `KeyProvider`. §A2.1 permits SHA-512 for
    /// §3.5.1's keyed-hash posture.
    HmacSha512,
}

/// The elide-builtin label PCI PAN templates dispatch on.
const PAN_LABEL: LabelRef = LabelRef::from_static("payment_card");

/// Elide-builtin labels PCI SAV templates dispatch on. Every
/// category §3.3.1 prohibits from post-authorization storage:
/// CVV/CVC (`card_security_code`), magnetic-stripe / chip
/// contents (`card_track_data`), and PIN blocks (`pin_block`).
const SAV_LABELS: &[LabelRef] = &[
    LabelRef::from_static("card_security_code"),
    LabelRef::from_static("card_track_data"),
    LabelRef::from_static("pin_block"),
];

/// PCI DSS §3.5.1 effective date (v4.0.1 mandatory compliance).
const EFFECTIVE_DATE: Date = Date::constant(2025, 3, 31);

const TRUNCATE_POLICY_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000001");
const TRUNCATE_RULE_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000002");
const HMAC_SHA256_POLICY_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000003");
const HMAC_SHA256_RULE_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000004");
const TRUNCATE_LAST_FOUR_POLICY_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000005");
const TRUNCATE_LAST_FOUR_RULE_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000006");
const HMAC_SHA512_POLICY_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000007");
const SAV_POLICY_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000009");
const SAV_RULE_ID: Uuid = uuid!("01958ccd-0000-7000-8000-00000000000a");
const HMAC_SHA512_RULE_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000008");

/// Per-render specification collapsed into the shape [`template`]
/// uses to fill in the shared shell.
struct RenderSpec {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    policy_id: Uuid,
    policy_name: &'static str,
    policy_description: &'static str,
    rule: PolicyRule,
}

/// Build the PCI DSS template for the picked subsection.
pub(crate) fn template(part: PciDssPart) -> Template {
    match part {
        PciDssPart::PanRender { render } => pan_template(render),
        PciDssPart::SavErase => sav_template(),
    }
}

/// PCI DSS §3.5.1 render template dispatched by `render`.
fn pan_template(render: PciPanRender) -> Template {
    let spec = spec(render);
    Template {
        id: spec.id.into(),
        name: spec.name.into(),
        version: Version::new(1, 0, 0),
        effective_date: EFFECTIVE_DATE,
        description: Some(spec.description.into()),
        policy: PolicyDefinition {
            id: spec.policy_id,
            name: spec.policy_name.into(),
            description: Some(spec.policy_description.to_owned()),
            when: None,
            labels: Labels {
                builtins: vec![PAN_LABEL.clone()],
                custom: Vec::new(),
            },
            groups: Vec::new(),
            rules: vec![spec.rule],
            fallback: None,
            retention: Vec::new(),
        },
    }
}

fn spec(render: PciPanRender) -> RenderSpec {
    match render {
        PciPanRender::Truncate => RenderSpec {
            id: "pci_dss_pan_truncate",
            name: "PCI DSS §3.5.1 PAN — truncate",
            description: "Render stored PAN unreadable via truncation, keeping the first six \
                          (BIN) and last four digits.",
            policy_id: TRUNCATE_POLICY_ID,
            policy_name: "pci-dss-pan-truncate",
            policy_description: "Truncate stored PAN to the first six digits and last four, \
                                 dropping the middle. Keeps BIN and last-four for downstream \
                                 lookups without leaving a reversible ciphertext or a key \
                                 surface to protect.",
            rule: truncate_rule(6, 4, TRUNCATE_RULE_ID, "pci-truncate-pan-bin-last-four"),
        },
        PciPanRender::TruncateLastFour => RenderSpec {
            id: "pci_dss_pan_truncate_last_four",
            name: "PCI DSS §3.5.1 PAN — truncate to last four",
            description: "Render stored PAN unreadable via truncation to the last four digits \
                          only. The stricter §3.5.1.1 posture (mandatory 2025-03-31) required \
                          when a downstream system also stores a hashed version of the same PAN.",
            policy_id: TRUNCATE_LAST_FOUR_POLICY_ID,
            policy_name: "pci-dss-pan-truncate-last-four",
            policy_description: "Truncate stored PAN to the last four digits, dropping BIN and \
                                 middle. Required by PCI DSS v4.0.1 §3.5.1.1 when the same \
                                 environment also stores a hashed version of the same PAN — \
                                 first-six + last-four coexistence with a hash requires \
                                 additional controls.",
            rule: truncate_rule(
                0,
                4,
                TRUNCATE_LAST_FOUR_RULE_ID,
                "pci-truncate-pan-last-four",
            ),
        },
        PciPanRender::HmacSha256 => RenderSpec {
            id: "pci_dss_pan_hmac_sha256",
            name: "PCI DSS §3.5.1 PAN — HMAC-SHA-256",
            description: "Render stored PAN unreadable via a keyed HMAC-SHA-256 digest. Requires \
                          the engine to have a KeyProvider wired.",
            policy_id: HMAC_SHA256_POLICY_ID,
            policy_name: "pci-dss-pan-hmac-sha256",
            policy_description: HMAC_POLICY_DESCRIPTION,
            rule: hmac_rule(
                Sha2Algorithm::Sha256,
                HMAC_SHA256_RULE_ID,
                "pci-hmac-pan-sha256",
                "SHA-256",
            ),
        },
        PciPanRender::HmacSha512 => RenderSpec {
            id: "pci_dss_pan_hmac_sha512",
            name: "PCI DSS §3.5.1 PAN — HMAC-SHA-512",
            description: "Render stored PAN unreadable via a keyed HMAC-SHA-512 digest. Requires \
                          the engine to have a KeyProvider wired.",
            policy_id: HMAC_SHA512_POLICY_ID,
            policy_name: "pci-dss-pan-hmac-sha512",
            policy_description: HMAC_POLICY_DESCRIPTION,
            rule: hmac_rule(
                Sha2Algorithm::Sha512,
                HMAC_SHA512_RULE_ID,
                "pci-hmac-pan-sha512",
                "SHA-512",
            ),
        },
    }
}

const HMAC_POLICY_DESCRIPTION: &str = "Replace stored PAN with a keyed HMAC digest. Requires the engine to have a KeyProvider \
     wired via `Engine::with_key_provider`. The key must stay secret; a leaked key permits \
     offline PAN enumeration against the shipped hash.";

fn truncate_rule(keep_prefix: usize, keep_suffix: usize, rule_id: Uuid, name: &str) -> PolicyRule {
    let description = if keep_prefix == 0 {
        format!("Drop everything but the last {keep_suffix} digits of every payment_card value.",)
    } else {
        format!(
            "Drop the middle of every payment_card value, keeping the first {keep_prefix} \
             (BIN) and last {keep_suffix} digits.",
        )
    };
    PolicyRule::Predicated(Box::new(PredicatedRule {
        id: rule_id,
        name: name.into(),
        description: Some(description),
        predicate: single_label(PAN_LABEL.clone()),
        action: ModalityRedactions::text(TextRedaction::Truncate {
            keep_prefix,
            keep_suffix,
        }),
    }))
}

fn hmac_rule(
    algorithm: Sha2Algorithm,
    rule_id: Uuid,
    name: &str,
    algorithm_label: &str,
) -> PolicyRule {
    PolicyRule::Predicated(Box::new(PredicatedRule {
        id: rule_id,
        name: name.into(),
        description: Some(format!(
            "Replace every payment_card value with an HMAC-{algorithm_label} digest keyed on \
             the engine's KeyProvider.",
        )),
        predicate: single_label(PAN_LABEL.clone()),
        action: ModalityRedactions::text(TextRedaction::HmacHash { algorithm }),
    }))
}

fn single_label(label: LabelRef) -> Predicate {
    Predicate::LabelOneOf {
        labels: vec![label],
    }
}

/// PCI DSS §3.3.1 — erase stored Sensitive Authentication Data
/// (SAV). Covers all three §3.3.1 categories: CVV/CVC
/// (`card_security_code`), magnetic-stripe / chip track data
/// (`card_track_data`), and PIN blocks (`pin_block`).
fn sav_template() -> Template {
    Template {
        id: "pci_dss_sav_erase".into(),
        name: "PCI DSS §3.3.1 SAV — erase".into(),
        version: Version::new(1, 0, 0),
        effective_date: EFFECTIVE_DATE,
        description: Some(
            "Erase Sensitive Authentication Data (CVV/CVC, track data, PIN blocks). \
             §3.3.1 prohibits storing SAV after authorization — the correct posture is \
             erasure, not render-unreadable."
                .into(),
        ),
        policy: PolicyDefinition {
            id: SAV_POLICY_ID,
            name: "pci-dss-sav-erase".into(),
            description: Some(
                "Erase every SAV entity — CVV/CVC, magnetic-stripe/chip track data, \
                 and PIN blocks. PCI DSS §3.3.1 forbids SAV storage after \
                 authorization completes; unlike PAN, SAV has no render-unreadable \
                 posture — it must be erased."
                    .to_owned(),
            ),
            when: None,
            labels: Labels {
                builtins: SAV_LABELS.to_vec(),
                custom: Vec::new(),
            },
            groups: Vec::new(),
            rules: vec![sav_rule()],
            fallback: None,
            retention: Vec::new(),
        },
    }
}

fn sav_rule() -> PolicyRule {
    PolicyRule::Predicated(Box::new(PredicatedRule {
        id: SAV_RULE_ID,
        name: "pci-sav-erase".into(),
        description: Some("Erase every SAV entity (CVV/CVC, track data, PIN blocks).".to_owned()),
        predicate: Predicate::LabelOneOf {
            labels: SAV_LABELS.to_vec(),
        },
        action: ModalityRedactions::text(TextRedaction::Erase),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &[PciPanRender] = &[
        PciPanRender::Truncate,
        PciPanRender::TruncateLastFour,
        PciPanRender::HmacSha256,
        PciPanRender::HmacSha512,
    ];

    #[test]
    fn every_render_targets_payment_card_label() {
        for render in ALL {
            let t = pan_template(*render);
            let PolicyRule::Predicated(rule) = &t.policy.rules[0] else {
                panic!("expected Predicated rule for {render:?}");
            };
            let Predicate::LabelOneOf { labels } = &rule.predicate else {
                panic!("expected LabelOneOf predicate for {render:?}");
            };
            assert_eq!(labels.len(), 1);
            assert_eq!(labels[0], PAN_LABEL);
        }
    }

    #[test]
    fn truncate_last_four_drops_the_bin() {
        let PolicyRule::Predicated(rule) =
            &pan_template(PciPanRender::TruncateLastFour).policy.rules[0]
        else {
            panic!("expected Predicated rule");
        };
        let TextRedaction::Truncate {
            keep_prefix,
            keep_suffix,
        } = rule.action.text.as_ref().unwrap()
        else {
            panic!("expected Truncate action");
        };
        assert_eq!(*keep_prefix, 0, "TruncateLastFour must drop BIN");
        assert_eq!(*keep_suffix, 4);
    }

    #[test]
    fn hmac_sha512_uses_sha512_algorithm() {
        let PolicyRule::Predicated(rule) = &pan_template(PciPanRender::HmacSha512).policy.rules[0]
        else {
            panic!("expected Predicated rule");
        };
        let TextRedaction::HmacHash { algorithm } = rule.action.text.as_ref().unwrap() else {
            panic!("expected HmacHash action");
        };
        assert_eq!(*algorithm, Sha2Algorithm::Sha512);
    }

    #[test]
    fn every_render_ships_a_distinct_policy_identity() {
        // Distinct template ids and policy ids across all four —
        // audits key on these to tell the postures apart, and
        // `TemplateCatalog::builtin()` inserts by (id, version)
        // so any collision silently drops one.
        let mut ids = std::collections::HashSet::new();
        let mut policy_ids = std::collections::HashSet::new();
        for render in ALL {
            let t = pan_template(*render);
            assert!(
                ids.insert(t.id.clone()),
                "duplicate template id for {render:?}: {}",
                t.id,
            );
            assert!(
                policy_ids.insert(t.policy.id),
                "duplicate policy id for {render:?}: {}",
                t.policy.id,
            );
        }
    }

    #[test]
    fn template_ids_are_stable_across_calls() {
        for render in ALL {
            let a = pan_template(*render);
            let b = pan_template(*render);
            assert_eq!(a.id, b.id);
            assert_eq!(a.policy.id, b.policy.id);
            assert_eq!(a.policy.rules[0].id(), b.policy.rules[0].id());
        }
    }

    #[test]
    fn sav_template_erases_every_sav_label() {
        let t = sav_template();
        let PolicyRule::Predicated(rule) = &t.policy.rules[0] else {
            panic!("expected Predicated rule");
        };
        let Predicate::LabelOneOf { labels } = &rule.predicate else {
            panic!("expected LabelOneOf predicate");
        };
        // Every §3.3.1 SAV label must be in the rule's predicate.
        for expected in SAV_LABELS {
            assert!(
                labels.contains(expected),
                "SAV rule missing label `{}`",
                expected.as_str(),
            );
        }
        assert!(matches!(rule.action.text, Some(TextRedaction::Erase)));
    }

    #[test]
    fn sav_and_pan_ship_distinct_identities() {
        // The SAV template must not collide with any PAN render on
        // template id or policy id — different regulatory subsection
        // (§3.3.1 vs §3.5.1), different label, different posture.
        let sav = sav_template();
        for render in ALL {
            let pan = pan_template(*render);
            assert_ne!(sav.id, pan.id, "sav vs pan template id collision");
            assert_ne!(
                sav.policy.id, pan.policy.id,
                "sav vs pan policy id collision"
            );
        }
    }
}
