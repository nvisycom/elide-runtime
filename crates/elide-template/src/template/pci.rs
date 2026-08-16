//! PCI DSS: Primary Account Number (PAN) and Sensitive
//! Authentication Data (SAV) postures.
//!
//! Two families ship from this module:
//!
//! ## §3.5.1: render stored PAN unreadable
//!
//! §3.5.1 lists four acceptable render approaches: one-way hashes
//! based on strong cryptography, truncation, index tokens with
//! securely stored pads, and strong cryptography with associated
//! key-management processes. This module ships four render
//! variants covering (a) and (b):
//!
//! - [`PciPanRender::Truncate`] → `Truncate { keep_prefix: 6, keep_suffix: 4 }`
//!   is the historical PCI truncation posture. Keeps BIN and
//!   last-four for downstream lookups. No key material involved.
//! - [`PciPanRender::TruncateLastFour`] → `Truncate { keep_prefix: 0, keep_suffix: 4 }`
//!   is the conservative truncation posture for environments that
//!   also store a hashed version of the same PAN. §3.5.1 requires
//!   controls preventing correlation between the hashed and
//!   truncated representations; dropping the BIN shrinks that
//!   correlation surface. Not itself a named requirement.
//! - [`PciPanRender::HmacSha256`] → `HmacHash { algorithm: Sha256 }`
//!   is the keyed-hash posture §3.5.1.1 mandates (effective
//!   2025-03-31): hashes rendering PAN unreadable must be keyed
//!   cryptographic hashes of the entire PAN, so an unkeyed digest
//!   no longer satisfies §3.5.1. Requires the engine to have a
//!   `KeyProvider` wired.
//! - [`PciPanRender::HmacSha512`] → `HmacHash { algorithm: Sha512 }`
//!   is the same posture with SHA-512. PCI DSS's "strong cryptography"
//!   glossary definition covers the SHA-2 family; both qualify.
//!
//! All render variants target the elide-builtin `payment_card`
//! label. Each template declares its own local [`LabelScope`].
//! Callers wanting more than one dispatched from one policy
//! compose the [`PolicyDefinition`]s themselves.
//!
//! ## §3.3.1: never store Sensitive Authentication Data
//!
//! §3.3.1 prohibits storage of SAV after authorization completes
//! (CVV/CVC, track data, PIN blocks). Unlike PAN, the correct
//! posture is not "render unreadable" but *erase*: SAV is never
//! allowed to persist. [`sav_template`] ships a single-posture
//! template targeting every elide-builtin SAV label
//! (`card_security_code`, `card_track_data`, `pin_block`) with
//! plain [`Erase`].
//!
//! [`Erase`]: elide_governance::redaction::TextRedaction::Erase
//!
//! [`LabelScope`]: elide_governance::LabelScope
//! [`PolicyDefinition`]: elide_governance::PolicyDefinition

use elide_core::entity::LabelRef;
use elide_governance::redaction::{ModalityRedactions, TextRedaction};
use elide_governance::{LabelScope, PolicyDefinition};
use elide_operator::operators::Sha2Algorithm;
use jiff::civil::Date;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::{Uuid, uuid};

use super::{Template, cited, origin};

/// Which PCI DSS subsection this template addresses.
///
/// - [`PanRender`](Self::PanRender): §3.5.1 render posture for
///   stored Primary Account Numbers. Carries a [`PciPanRender`]
///   picking between the shipped render approaches.
/// - [`SavErase`](Self::SavErase): §3.3.1 prohibition on
///   storing Sensitive Authentication Data (CVV/CVC, track data,
///   PIN blocks) after authorization. No options: SAV has one
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
/// [`Engine::with_key_provider`]: https://docs.rs/elide-pipeline/latest/elide_pipeline/struct.Engine.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PciPanRender {
    /// Truncate stored PAN to the first six (BIN) and last four
    /// digits.
    Truncate,
    /// Truncate stored PAN to the last four digits only (BIN
    /// dropped). The conservative posture where a hashed copy of
    /// the same PAN coexists: §3.5.1 requires controls preventing
    /// the two representations from being correlated back to the
    /// original, and dropping the BIN shrinks that surface. Not a
    /// named requirement: §3.5.1.1 governs hashing, not
    /// truncation.
    TruncateLastFour,
    /// Replace stored PAN with an HMAC-SHA-256 digest keyed on
    /// the engine's `KeyProvider`. Satisfies §3.5.1.1 (effective
    /// 2025-03-31), which requires hashes rendering PAN unreadable
    /// to be keyed cryptographic hashes of the entire PAN.
    HmacSha256,
    /// Replace stored PAN with an HMAC-SHA-512 digest keyed on
    /// the engine's `KeyProvider`. Same §3.5.1.1 posture as
    /// [`HmacSha256`](Self::HmacSha256) with a wider digest; PCI
    /// DSS's "strong cryptography" definition covers the SHA-2
    /// family.
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

/// PCI DSS v4.0 effective date. The base for requirements that
/// are not future-dated: §3.3.1's SAV prohibition (a core
/// requirement since v1.0) and §3.5.1's truncation approaches.
const V4_EFFECTIVE_DATE: Date = Date::constant(2022, 3, 31);

/// §3.5.1.1 effective date: the keyed-hash mandate is
/// future-dated and became mandatory on this day. Applies only to
/// the HMAC render variants.
const KEYED_HASH_EFFECTIVE_DATE: Date = Date::constant(2025, 3, 31);

const TRUNCATE_POLICY_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000001");
const HMAC_SHA256_POLICY_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000003");
const TRUNCATE_LAST_FOUR_POLICY_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000005");
const HMAC_SHA512_POLICY_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000007");
const SAV_POLICY_ID: Uuid = uuid!("01958ccd-0000-7000-8000-000000000009");

/// Per-render specification collapsed into the shape [`template`]
/// uses to fill in the shared shell.
struct RenderSpec {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    policy_id: Uuid,
    policy_name: &'static str,
    policy_description: &'static str,
    /// The scope this variant declares, carrying the provision that
    /// makes *this* render posture compliant. Each variant cites a
    /// different one, which is why the citation lives here rather
    /// than on a shared `pci_pan` scope.
    scope: LabelScope,
    /// What to do with everything the scope detects.
    action: TextRedaction,
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
    // Only the keyed-hash variants carry §3.5.1.1's future-dated
    // mandate; truncation has been available since v4.0.
    let effective_date = match render {
        PciPanRender::HmacSha256 | PciPanRender::HmacSha512 => KEYED_HASH_EFFECTIVE_DATE,
        PciPanRender::Truncate | PciPanRender::TruncateLastFour => V4_EFFECTIVE_DATE,
    };
    Template {
        id: spec.id.into(),
        name: spec.name.into(),
        version: Version::new(1, 0, 0),
        effective_date,
        description: Some(spec.description.into()),
        policy: PolicyDefinition {
            id: spec.policy_id,
            name: spec.policy_name.into(),
            description: Some(spec.policy_description.into()),
            template: Some(origin(spec.id, Version::new(1, 0, 0))),
            scopes: vec![spec.scope],
            custom: Vec::new(),
            // No rules: the scope is one label and the whole point
            // of the variant is what happens to it, so the fallback
            // carries the action and inherits the scope's citation.
            rules: Vec::new(),
            fallback: Some(ModalityRedactions::text(spec.action)),
        },
    }
}

/// A PAN scope named `name`, citing the provision that makes this
/// render posture compliant.
///
/// Every variant covers the same single label; only the citation
/// differs, which is exactly why each declares its own scope rather
/// than sharing one.
fn pan_scope(name: &'static str, citation: &'static str, rationale: &'static str) -> LabelScope {
    LabelScope::new(name, [PAN_LABEL.clone()])
        .with_description("Stored Primary Account Numbers.")
        .with_attribution(cited("PCI DSS", citation, rationale))
}

fn spec(render: PciPanRender) -> RenderSpec {
    match render {
        PciPanRender::Truncate => RenderSpec {
            id: "pci_dss_pan_truncate",
            name: "PCI DSS §3.5.1 PAN: truncate",
            description: "Render stored PAN unreadable via truncation, keeping the first six \
                          (BIN) and last four digits.",
            policy_id: TRUNCATE_POLICY_ID,
            policy_name: "pci-dss-pan-truncate",
            policy_description: "Truncate stored PAN to the first six digits and last four, \
                                 dropping the middle. Keeps BIN and last-four for downstream \
                                 lookups without leaving a reversible ciphertext or a key \
                                 surface to protect.",
            scope: pan_scope(
                "pci_pan_truncate",
                "§3.5.1",
                "truncation is one of the approaches §3.5.1 permits for rendering \
                 stored PAN unreadable",
            ),
            action: TextRedaction::Truncate {
                keep_prefix: 6,
                keep_suffix: 4,
            },
        },
        PciPanRender::TruncateLastFour => RenderSpec {
            id: "pci_dss_pan_truncate_last_four",
            name: "PCI DSS §3.5.1 PAN: truncate to last four",
            description: "Render stored PAN unreadable via truncation to the last four digits \
                          only. The conservative posture where a hashed copy of the same PAN \
                          also exists in the environment.",
            policy_id: TRUNCATE_LAST_FOUR_POLICY_ID,
            policy_name: "pci-dss-pan-truncate-last-four",
            policy_description: "Truncate stored PAN to the last four digits, dropping BIN and \
                                 middle. PCI DSS §3.5.1 requires controls preventing hashed and \
                                 truncated versions of one PAN from being correlated to \
                                 reconstruct it; dropping the BIN shrinks that surface. Not a \
                                 named requirement: §3.5.1.1 governs hashing, not truncation.",
            scope: pan_scope(
                "pci_pan_truncate_last_four",
                "§3.5.1",
                "truncation is one of the approaches §3.5.1 permits; dropping the BIN \
                 shrinks the surface for correlating a truncated PAN with a hashed one",
            ),
            action: TextRedaction::Truncate {
                keep_prefix: 0,
                keep_suffix: 4,
            },
        },
        PciPanRender::HmacSha256 => RenderSpec {
            id: "pci_dss_pan_hmac_sha256",
            name: "PCI DSS §3.5.1 PAN: HMAC-SHA-256",
            description: "Render stored PAN unreadable via a keyed HMAC-SHA-256 digest. Requires \
                          the engine to have a KeyProvider wired.",
            policy_id: HMAC_SHA256_POLICY_ID,
            policy_name: "pci-dss-pan-hmac-sha256",
            policy_description: HMAC_POLICY_DESCRIPTION,
            scope: pan_scope(
                "pci_pan_hmac_sha256",
                "§3.5.1.1",
                "a hash rendering stored PAN unreadable must be a keyed \
                 cryptographic hash of the entire PAN",
            ),
            action: TextRedaction::HmacHash {
                algorithm: Sha2Algorithm::Sha256,
            },
        },
        PciPanRender::HmacSha512 => RenderSpec {
            id: "pci_dss_pan_hmac_sha512",
            name: "PCI DSS §3.5.1 PAN: HMAC-SHA-512",
            description: "Render stored PAN unreadable via a keyed HMAC-SHA-512 digest. Requires \
                          the engine to have a KeyProvider wired.",
            policy_id: HMAC_SHA512_POLICY_ID,
            policy_name: "pci-dss-pan-hmac-sha512",
            policy_description: HMAC_POLICY_DESCRIPTION,
            scope: pan_scope(
                "pci_pan_hmac_sha512",
                "§3.5.1.1",
                "a hash rendering stored PAN unreadable must be a keyed \
                 cryptographic hash of the entire PAN",
            ),
            action: TextRedaction::HmacHash {
                algorithm: Sha2Algorithm::Sha512,
            },
        },
    }
}

const HMAC_POLICY_DESCRIPTION: &str = "Replace stored PAN with a keyed HMAC digest. Satisfies PCI DSS §3.5.1.1 (mandatory \
     2025-03-31), which requires hashes rendering PAN unreadable to be keyed cryptographic \
     hashes of the entire PAN: an unkeyed digest no longer satisfies §3.5.1. Requires the \
     engine to have a KeyProvider wired via `Engine::with_key_provider`. The key must stay \
     secret; a leaked key permits offline PAN enumeration against the shipped hash.";

/// PCI DSS §3.3.1: erase stored Sensitive Authentication Data
/// (SAV). Covers all three §3.3.1 categories: CVV/CVC
/// (`card_security_code`), magnetic-stripe / chip track data
/// (`card_track_data`), and PIN blocks (`pin_block`).
fn sav_template() -> Template {
    Template {
        id: "pci_dss_sav_erase".into(),
        name: "PCI DSS §3.3.1 SAV: erase".into(),
        version: Version::new(1, 0, 0),
        effective_date: V4_EFFECTIVE_DATE,
        description: Some(
            "Erase Sensitive Authentication Data (CVV/CVC, track data, PIN blocks). \
             §3.3.1 prohibits storing SAV after authorization: the correct posture is \
             erasure, not render-unreadable."
                .into(),
        ),
        policy: PolicyDefinition {
            id: SAV_POLICY_ID,
            name: "pci-dss-sav-erase".into(),
            template: Some(origin("pci_dss_sav_erase", Version::new(1, 0, 0))),
            description: Some(
                "Erase every SAV entity: CVV/CVC, magnetic-stripe/chip track data, \
                 and PIN blocks. PCI DSS §3.3.1 forbids SAV storage after \
                 authorization completes; unlike PAN, SAV has no render-unreadable \
                 posture: it must be erased."
                    .into(),
            ),
            scopes: vec![
                LabelScope::new("pci_sav", SAV_LABELS.to_vec())
                    .with_description(
                        "Sensitive authentication data: CVV/CVC, magnetic-stripe and \
                         chip track data, and PIN blocks.",
                    )
                    .with_attribution(cited(
                        "PCI DSS",
                        "§3.3.1",
                        "sensitive authentication data must not be retained after \
                         authorization completes",
                    )),
            ],
            custom: Vec::new(),
            // No rules: §3.3.1 admits one posture for every SAV
            // category, so the fallback expresses it and inherits
            // the scope's citation. Unlike §3.5.1, there is no
            // render choice to cite per-variant.
            rules: Vec::new(),
            fallback: Some(ModalityRedactions::text(TextRedaction::Erase)),
        },
    }
}

#[cfg(test)]
mod tests {
    use elide_core::entity::audit::AttributionKind;

    use super::*;

    const ALL: &[PciPanRender] = &[
        PciPanRender::Truncate,
        PciPanRender::TruncateLastFour,
        PciPanRender::HmacSha256,
        PciPanRender::HmacSha512,
    ];

    #[test]
    fn only_keyed_hash_variants_carry_the_future_dated_effective_date() {
        // Reviewers check `effective_date` against the run date to
        // confirm the template that fired was in force at the time.
        // §3.5.1.1's keyed-hash mandate is future-dated to
        // 2025-03-31; truncation and the §3.3.1 SAV prohibition are
        // not, so stamping them with 2025 would imply no obligation
        // existed before then.
        for render in ALL {
            let want = match render {
                PciPanRender::HmacSha256 | PciPanRender::HmacSha512 => KEYED_HASH_EFFECTIVE_DATE,
                PciPanRender::Truncate | PciPanRender::TruncateLastFour => V4_EFFECTIVE_DATE,
            };
            assert_eq!(
                pan_template(*render).effective_date,
                want,
                "{render:?} carries the wrong effective date",
            );
        }
        assert_eq!(sav_template().effective_date, V4_EFFECTIVE_DATE);
    }

    #[test]
    fn every_render_targets_payment_card_label() {
        for render in ALL {
            let t = pan_template(*render);
            assert_eq!(t.policy.scopes.len(), 1, "{render:?} declares one scope");
            assert_eq!(t.policy.scopes[0].labels, vec![PAN_LABEL.clone()]);
        }
    }

    #[test]
    fn every_render_cites_the_provision_its_posture_answers_to() {
        // Truncation is permitted by §3.5.1; a keyed hash is what
        // §3.5.1.1 mandates. The citation rides on each variant's
        // own scope, so the fallback inherits the right one.
        for render in ALL {
            let want = match render {
                PciPanRender::HmacSha256 | PciPanRender::HmacSha512 => "§3.5.1.1",
                PciPanRender::Truncate | PciPanRender::TruncateLastFour => "§3.5.1",
            };
            let attribution = pan_template(*render).policy.scopes[0]
                .attribution
                .clone()
                .unwrap_or_else(|| panic!("{render:?} scope must cite its provision"));
            let AttributionKind::Cited { citation, .. } = attribution else {
                panic!("{render:?} must carry a Cited attribution");
            };
            assert_eq!(citation, want, "{render:?} cites the wrong provision");
        }
    }

    #[test]
    fn truncate_last_four_drops_the_bin() {
        let fallback = pan_template(PciPanRender::TruncateLastFour)
            .policy
            .fallback
            .expect("the render posture rides on the fallback");
        let Some(TextRedaction::Truncate {
            keep_prefix,
            keep_suffix,
        }) = fallback.text
        else {
            panic!("expected Truncate action");
        };
        assert_eq!(keep_prefix, 0, "TruncateLastFour must drop BIN");
        assert_eq!(keep_suffix, 4);
    }

    #[test]
    fn hmac_sha512_uses_sha512_algorithm() {
        let fallback = pan_template(PciPanRender::HmacSha512)
            .policy
            .fallback
            .expect("the render posture rides on the fallback");
        let Some(TextRedaction::HmacHash { algorithm }) = fallback.text else {
            panic!("expected HmacHash action");
        };
        assert_eq!(algorithm, Sha2Algorithm::Sha512);
    }

    #[test]
    fn every_render_ships_a_distinct_policy_identity() {
        // Distinct template ids and policy ids across all four -
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
        }
    }

    #[test]
    fn sav_template_erases_every_sav_label() {
        let t = sav_template();
        // Every §3.3.1 SAV label must be in scope, since the
        // fallback acts on whatever the scope detects.
        for expected in SAV_LABELS {
            assert!(
                t.policy.scopes[0].labels.contains(expected),
                "SAV scope missing label `{}`",
                expected.as_str(),
            );
        }
        let fallback = t.policy.fallback.expect("SAV erases via the fallback");
        assert!(matches!(fallback.text, Some(TextRedaction::Erase)));
    }

    #[test]
    fn sav_and_pan_ship_distinct_identities() {
        // The SAV template must not collide with any PAN render on
        // template id or policy id: different regulatory subsection
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
