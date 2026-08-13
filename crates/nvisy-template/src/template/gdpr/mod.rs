//! GDPR Article 9 — special categories of personal data, with
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
//! across mentions — pseudonymization rather than erasure.
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
//!   adds `date_of_birth` and `postal_code` to defeat
//!   re-identification joins under Recital 26 guidance; and
//!   [`Article9And10`](GdprSensitiveScope::Article9And10) further
//!   adds Article 10's criminal-justice labels
//!   (`criminal_record`, `criminal_charge`, `judicial_narrative`).
//!
//! [`Predicated`]: nvisy_policy::RuleDispatch::Predicated

use elide_core::entity::LabelRef;
use jiff::civil::Date;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Template;

mod erase;
mod pseudonymize;
mod sensitive_scope;

pub use self::sensitive_scope::GdprSensitiveScope;

/// GDPR Regulation (EU) 2016/679 effective date.
pub(super) const EFFECTIVE_DATE: Date = Date::constant(2018, 5, 25);

/// Which operator to apply to Article 9 special-category entities.
///
/// - [`Erase`](Self::Erase) — the default no-lawful-basis posture.
///   Every match is removed.
/// - [`Pseudonymize`](Self::Pseudonymize) — identity-preserving
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

/// The GDPR Article 9 template config — treatment axis fused
/// with sensitive-scope axis. Carried directly by
/// [`crate::PolicyTemplate::GdprArticle9`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GdprArticle9 {
    /// Which operator to apply to matches. See
    /// [`GdprArticle9Treatment`] for the tradeoff.
    pub treatment: GdprArticle9Treatment,
    /// Which sensitive-data labels to cover. Defaults to
    /// [`GdprSensitiveScope::Article9`] (nine Article 9(1)
    /// categories only). Pick
    /// [`GdprSensitiveScope::Article9WithReidHardening`] to add
    /// Recital 26 quasi-identifiers, or
    /// [`GdprSensitiveScope::Article9And10`] to also cover
    /// Article 10 criminal-justice data.
    #[serde(default)]
    pub scope: GdprSensitiveScope,
}

impl GdprArticle9 {
    /// Build the Article 9 template for this config.
    pub(crate) fn template(self) -> Template {
        match self.treatment {
            GdprArticle9Treatment::Erase => erase::template(self),
            GdprArticle9Treatment::Pseudonymize => pseudonymize::template(self),
        }
    }

    /// The full label set this config covers. Delegates to
    /// `scope.labels()` since the treatment axis doesn't affect
    /// label membership; kept as a method on the config type for
    /// symmetry with the HIPAA config.
    pub(super) fn labels(self) -> Vec<LabelRef> {
        self.scope.labels()
    }
}

const fn article_9_group_description() -> &'static str {
    "The nine special categories of personal data enumerated in GDPR Article 9(1) \
     (racial/ethnic origin, political opinions, religious/philosophical beliefs, \
     trade-union membership, genetic data, biometric data for unique identification, \
     health data, sex life, sexual orientation). Scope may widen with Recital 26 \
     quasi-identifiers and Article 10 criminal-justice labels."
}

#[cfg(test)]
mod tests {
    use nvisy_policy::RuleDispatch;
    use nvisy_policy::redaction::TextRedaction;

    use super::sensitive_scope::GDPR_LABELS;
    use super::*;

    fn cfg(treatment: GdprArticle9Treatment, scope: GdprSensitiveScope) -> GdprArticle9 {
        GdprArticle9 { treatment, scope }
    }

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
        let template = cfg(GdprArticle9Treatment::Erase, GdprSensitiveScope::default()).template();
        let RuleDispatch::Predicated { action, .. } = &template.policy.rules[0].dispatch else {
            panic!("expected Predicated dispatch");
        };
        assert!(matches!(action.text, Some(TextRedaction::Erase)));
    }

    #[test]
    fn pseudonymize_treatment_uses_pseudonymize_action() {
        let template = cfg(
            GdprArticle9Treatment::Pseudonymize,
            GdprSensitiveScope::default(),
        )
        .template();
        let RuleDispatch::Predicated { action, .. } = &template.policy.rules[0].dispatch else {
            panic!("expected Predicated dispatch");
        };
        assert!(matches!(action.text, Some(TextRedaction::Pseudonymize)));
    }

    #[test]
    fn treatments_ship_distinct_policy_identities() {
        let scope = GdprSensitiveScope::default();
        let e = cfg(GdprArticle9Treatment::Erase, scope).template();
        let p = cfg(GdprArticle9Treatment::Pseudonymize, scope).template();
        assert_ne!(e.id, p.id);
        assert_ne!(e.policy.id, p.policy.id);
    }

    #[test]
    fn template_ids_are_stable_across_calls() {
        let scope = GdprSensitiveScope::default();
        for treatment in [
            GdprArticle9Treatment::Erase,
            GdprArticle9Treatment::Pseudonymize,
        ] {
            let a = cfg(treatment, scope).template();
            let b = cfg(treatment, scope).template();
            assert_eq!(a.id, b.id);
            assert_eq!(a.policy.id, b.policy.id);
            assert_eq!(a.policy.rules[0].id, b.policy.rules[0].id);
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
            "date_of_birth",      // Recital 26
            "postal_code",        // Recital 26
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
        for want in ["date_of_birth", "postal_code"] {
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
        // broader scope — else a caller upgrading through the
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
