//! GDPR Article 9: special categories of personal data, with
//! optional extensions to Article 10 and Recital 26.
//!
//! Article 9(1) prohibits processing of nine categories of
//! personal data by default (racial/ethnic origin, political
//! opinions, religious/philosophical beliefs, trade-union
//! membership, genetic data, biometric data used to uniquely
//! identify a person, health data, sex life, sexual orientation).
//!
//! Article 9(2) enumerates ten lawful-basis carve-outs (explicit
//! consent, employment law, vital interests, public interest in
//! public health, ...) that permit processing. Callers invoking
//! one of those carve-outs commonly need to retain the
//! special-category data with per-entity identity preserved
//! across mentions: pseudonymization rather than erasure.
//!
//! Two shape axes:
//!
//! - [`GdprArticle9Treatment`] picks between the two shipped
//!   postures: [`Erase`](GdprArticle9Treatment::Erase) for the
//!   default no-lawful-basis posture, and
//!   [`Pseudonymize`](GdprArticle9Treatment::Pseudonymize) for
//!   the carve-out-backed retention posture.
//! - [`GdprSensitiveScope`] widens the label set: the default
//!   [`Article9`](GdprSensitiveScope::Article9) covers the nine
//!   Article 9(1) categories only;
//!   [`Article9WithReidHardening`](GdprSensitiveScope::Article9WithReidHardening)
//!   adds a product-defined quasi-identifier set to raise the cost
//!   of re-identification joins; and
//!   [`Article9And10`](GdprSensitiveScope::Article9And10) further
//!   adds Article 10's criminal-justice labels
//!   (`criminal_record`, `criminal_charge`, `judicial_narrative`).
//!
//! [`Predicated`]: elide_governance::RuleDispatch::Predicated

use elide_core::entity::audit::AttributionKind;
use jiff::civil::Date;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Template, cited};

mod erase;
mod pseudonymize;
mod sensitive_scope;

pub use self::sensitive_scope::GdprSensitiveScope;

/// GDPR Regulation (EU) 2016/679 effective date.
pub(super) const EFFECTIVE_DATE: Date = Date::constant(2018, 5, 25);

/// Which operator to apply to Article 9 special-category entities.
///
/// - [`Erase`](Self::Erase): the default no-lawful-basis posture.
///   Every match is removed.
/// - [`Pseudonymize`](Self::Pseudonymize): identity-preserving
///   surrogate. Suitable when an Article 9(2) carve-out
///   (explicit consent, employment law, public-health public
///   interest, ...) authorizes retention and downstream
///   analytics need per-entity coreference across mentions. The
///   9(2) basis itself remains the caller's out-of-band
///   obligation to establish and document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GdprArticle9Treatment {
    /// Erase every match. The default posture when no Article
    /// 9(2) carve-out applies.
    #[default]
    Erase,
    /// Pseudonymize every match (identity-preserving surrogate).
    /// Requires an Article 9(2) lawful-basis carve-out
    /// established out-of-band.
    Pseudonymize,
}

impl GdprArticle9Treatment {
    /// Every shipped treatment.
    ///
    /// Exhaustive by construction: adding a variant without adding
    /// it here leaves `_exhaustive` non-exhaustive, so the compiler
    /// catches the omission rather than a test silently covering
    /// one fewer posture.
    pub const ALL: &[Self] = &[Self::Erase, Self::Pseudonymize];

    /// Compile-time proof that [`ALL`](Self::ALL) lists every
    /// variant. Never called.
    const fn _exhaustive(self) {
        match self {
            Self::Erase | Self::Pseudonymize => {}
        }
    }
}

/// Build the Article 9 template for `treatment` over `scope`.
/// Dispatched from [`crate::PolicyTemplate::GdprArticle9`].
pub(crate) fn template(treatment: GdprArticle9Treatment, scope: GdprSensitiveScope) -> Template {
    match treatment {
        GdprArticle9Treatment::Erase => erase::template(scope),
        GdprArticle9Treatment::Pseudonymize => pseudonymize::template(scope),
    }
}

/// The Article 9(1) authority both postures' groups answer to.
/// Shared so erase and pseudonymize cite it identically.
fn article_9_attribution() -> AttributionKind {
    cited(
        "GDPR",
        "Article 9(1)",
        "special categories of personal data: processing prohibited by default, \
         absent an Article 9(2) carve-out",
    )
}

const fn article_9_group_description() -> &'static str {
    "The nine special categories of personal data enumerated in GDPR Article 9(1) \
     (racial/ethnic origin, political opinions, religious/philosophical beliefs, \
     trade-union membership, genetic data, biometric data for unique identification, \
     health data, sex life, sexual orientation). Scope may widen with \
     re-identification quasi-identifiers and Article 10 criminal-justice labels."
}

/// `base` with the sensitive scope folded in.
///
/// The scopes emit materially different label sets (Article 9(1)
/// alone versus Article 9 plus quasi-identifiers plus Article 10
/// criminal-justice data), so they must not share a template id: an
/// audit keyed on one could not tell whether criminal-justice data
/// was in scope. `Article9` is the default and appends nothing, so
/// its ids stay as originally shipped.
pub(super) fn template_id(base: &str, scope: GdprSensitiveScope) -> String {
    match scope {
        GdprSensitiveScope::Article9 => base.to_owned(),
        GdprSensitiveScope::Article9WithReidHardening => format!("{base}_reid_hardened"),
        GdprSensitiveScope::Article9And10 => format!("{base}_with_article_10"),
    }
}

#[cfg(test)]
mod tests {
    use elide_governance::redaction::TextRedaction;

    use super::sensitive_scope::GDPR_LABELS;
    use super::*;

    #[test]
    fn every_article_9_category_has_at_least_one_label() {
        // Spot-check one label per category so a future edit that
        // drops a whole category (say, deletes every biometric)
        // trips this rather than silently regressing coverage.
        for anchor in [
            "ethnicity",
            "political_opinion",
            "religion",
            "trade_union_membership",
            "genetic_data",
            "fingerprint",
            "medical_id",
            "health_narrative",
            "sex_life",
            "sexual_orientation",
        ] {
            assert!(
                GDPR_LABELS.iter().any(|l| l.as_str() == anchor),
                "expected anchor label `{anchor}` in Article 9 group",
            );
        }
    }

    #[test]
    fn erase_treatment_uses_erase_action() {
        // The whole scope gets one treatment, carried by the
        // fallback rather than a rule.
        let built = template(GdprArticle9Treatment::Erase, GdprSensitiveScope::default());
        let fallback = built
            .policy
            .fallback
            .expect("erase posture sets a fallback");
        assert!(matches!(fallback.text, Some(TextRedaction::Erase)));
    }

    #[test]
    fn pseudonymize_treatment_uses_pseudonymize_action() {
        let built = template(
            GdprArticle9Treatment::Pseudonymize,
            GdprSensitiveScope::default(),
        );
        let fallback = built
            .policy
            .fallback
            .expect("pseudonymize posture sets a fallback");
        assert!(matches!(fallback.text, Some(TextRedaction::Pseudonymize)));
    }

    #[test]
    fn treatments_ship_distinct_policy_identities() {
        let scope = GdprSensitiveScope::default();
        let e = template(GdprArticle9Treatment::Erase, scope);
        let p = template(GdprArticle9Treatment::Pseudonymize, scope);
        assert_ne!(e.id, p.id);
        assert_ne!(e.policy.id, p.policy.id);
    }

    #[test]
    fn every_treatment_scope_pair_has_a_distinct_identity() {
        // Scope changes what the policy covers: Article 9 alone is
        // 17 labels, Article 9 + 10 is 28. Sharing an id across
        // those would leave an audit unable to tell whether
        // criminal-justice data was in scope.
        let mut seen = std::collections::HashSet::new();
        for &treatment in GdprArticle9Treatment::ALL {
            for &scope in GdprSensitiveScope::ALL {
                let built = template(treatment, scope);
                assert!(
                    seen.insert(built.id.clone()),
                    "template id `{}` repeats across configurations",
                    built.id,
                );
                assert!(
                    seen.insert(built.policy.id.to_string().into()),
                    "policy UUID repeats for {treatment:?} / {scope:?}",
                );
                for rule in &built.policy.rules {
                    assert!(
                        seen.insert(rule.id.to_string().into()),
                        "rule UUID repeats for {treatment:?} / {scope:?}",
                    );
                }
                for declared in &built.policy.scopes {
                    assert!(
                        !declared.labels.is_empty(),
                        "{treatment:?} / {scope:?} declares an empty scope",
                    );
                }
            }
        }
    }

    #[test]
    fn identities_are_reproducible_across_calls() {
        // v5 derivation must be stable: an id that shifts between
        // builds would break every audit that recorded the old one.
        for scope in [
            GdprSensitiveScope::Article9,
            GdprSensitiveScope::Article9And10,
        ] {
            let a = template(GdprArticle9Treatment::Erase, scope);
            let b = template(GdprArticle9Treatment::Erase, scope);
            assert_eq!(a.id, b.id);
            assert_eq!(a.policy.id, b.policy.id);
        }
    }

    #[test]
    fn policies_record_the_template_they_came_from() {
        // Provenance: a policy built from a template names it, so a
        // reviewer holding only the policy can tell it apart from a
        // hand-authored one.
        let built = template(GdprArticle9Treatment::Erase, GdprSensitiveScope::default());
        let origin = built
            .policy
            .template
            .as_ref()
            .expect("a template-built policy must record its origin");
        assert_eq!(origin.id, built.id);
        assert_eq!(origin.version, built.version);
    }

    #[test]
    fn template_ids_are_stable_across_calls() {
        let scope = GdprSensitiveScope::default();
        for &treatment in GdprArticle9Treatment::ALL {
            let a = template(treatment, scope);
            let b = template(treatment, scope);
            assert_eq!(a.id, b.id);
            assert_eq!(a.policy.id, b.policy.id);
        }
    }

    #[test]
    fn default_scope_is_article_9_only() {
        // The default scope must be Article 9 alone so existing
        // callers upgrading from the pre-scope API see zero
        // behavior change.
        assert_eq!(GdprSensitiveScope::default(), GdprSensitiveScope::Article9);
        let labels = GdprSensitiveScope::Article9.labels();
        for outside in [
            "date_of_birth", // quasi-identifier
            "postal_code",   // quasi-identifier
            "gender",        // quasi-identifier
            // Article 9(1) covers ethnic origin; nationality and
            // citizenship are legal statuses, so they belong to the
            // quasi-identifier tier rather than the default scope.
            "nationality",
            "citizenship",
            "criminal_record",    // Article 10
            "criminal_charge",    // Article 10
            "judicial_narrative", // Article 10
        ] {
            assert!(
                !labels.iter().any(|l| l.as_str() == outside),
                "default Article 9 scope must NOT include `{outside}`",
            );
        }
    }

    #[test]
    fn reid_hardening_adds_only_recital_26_labels() {
        let labels = GdprSensitiveScope::Article9WithReidHardening.labels();
        for want in [
            "date_of_birth",
            "postal_code",
            "gender",
            "age",
            "city",
            "nationality",
            "citizenship",
            "occupation",
        ] {
            assert!(
                labels.iter().any(|l| l.as_str() == want),
                "reid-hardening scope must include Recital 26 label `{want}`",
            );
        }
        for outside in ["criminal_record", "criminal_charge", "judicial_narrative"] {
            assert!(
                !labels.iter().any(|l| l.as_str() == outside),
                "reid-hardening scope must NOT include Article 10 label `{outside}`",
            );
        }
    }

    #[test]
    fn article_9_and_10_covers_recital_26_and_article_10() {
        let labels = GdprSensitiveScope::Article9And10.labels();
        for want in [
            "ethnicity",          // Article 9 anchor
            "date_of_birth",      // Recital 26
            "postal_code",        // Recital 26
            "criminal_record",    // Article 10
            "criminal_charge",    // Article 10
            "judicial_narrative", // Article 10
        ] {
            assert!(
                labels.iter().any(|l| l.as_str() == want),
                "Article 9 + 10 scope must include `{want}`",
            );
        }
    }

    #[test]
    fn scopes_are_strictly_widening() {
        // Every label in the narrower scope must appear in the
        // broader scope: else a caller upgrading through the
        // tiers would lose coverage, defeating the point of the
        // ordered widening.
        let article_9 = GdprSensitiveScope::Article9.labels();
        let reid = GdprSensitiveScope::Article9WithReidHardening.labels();
        let both = GdprSensitiveScope::Article9And10.labels();
        for label in &article_9 {
            assert!(reid.contains(label), "Recital 26 tier missing `{label:?}`");
            assert!(both.contains(label), "Article 10 tier missing `{label:?}`");
        }
        for label in &reid {
            assert!(both.contains(label), "Article 10 tier missing `{label:?}`");
        }
    }
}
