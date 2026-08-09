//! GDPR Article 9 — special categories of personal data.
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
//! [`GdprArticle9Treatment`] picks between the two shipped
//! postures: [`Erase`](GdprArticle9Treatment::Erase) for the
//! default no-lawful-basis posture, and
//! [`Pseudonymize`](GdprArticle9Treatment::Pseudonymize) for the
//! carve-out-backed retention posture.
//!
//! [`Predicated`]: nvisy_policy::PolicyRule::Predicated

use elide_core::entity::LabelRef;
use jiff::civil::Date;
use nvisy_policy::predicate::Predicate;
use nvisy_policy::redaction::{ModalityRedactions, TextRedaction};
use nvisy_policy::{LabelGroup, Labels, PolicyDefinition, PolicyRule, PredicatedRule};
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::{Uuid, uuid};

use super::Template;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GdprArticle9Treatment {
    /// Erase every match. The default posture when no Article
    /// 9(2) carve-out applies.
    Erase,
    /// Pseudonymize every match (identity-preserving surrogate).
    /// Requires an Article 9(2) lawful-basis carve-out
    /// established out-of-band.
    Pseudonymize,
}

/// Group name Erase's bulk rule references.
const ERASE_GROUP: &str = "gdpr_article_9_erase";
/// Group name Pseudonymize's bulk rule references. Separate name
/// so audits distinguish the two postures by group id alone.
const PSEUDONYMIZE_GROUP: &str = "gdpr_article_9_pseudonymize";

/// Elide-builtin labels the group covers, mapped from Article
/// 9(1) categories.
const GDPR_LABELS: &[LabelRef] = &[
    // Racial or ethnic origin
    LabelRef::from_static("ethnicity"),
    LabelRef::from_static("nationality"),
    // Political opinions
    LabelRef::from_static("political_opinion"),
    // Religious or philosophical beliefs
    LabelRef::from_static("religion"),
    // Trade-union membership
    LabelRef::from_static("trade_union_membership"),
    // Genetic data
    LabelRef::from_static("genetic_data"),
    // Biometric data (used for unique identification)
    LabelRef::from_static("fingerprint"),
    LabelRef::from_static("voiceprint"),
    LabelRef::from_static("retina_scan"),
    LabelRef::from_static("facial_geometry"),
    // Health data — specific identifiers plus the broader
    // Article 4(15) health-narrative catch-all (blood pressure,
    // appointment notes, therapy references, care plans).
    LabelRef::from_static("medical_id"),
    LabelRef::from_static("insurance_id"),
    LabelRef::from_static("prescription_id"),
    LabelRef::from_static("diagnosis"),
    LabelRef::from_static("medication"),
    LabelRef::from_static("health_narrative"),
    // Sex life and sexual orientation
    LabelRef::from_static("sex_life"),
    LabelRef::from_static("sexual_orientation"),
];

const EFFECTIVE_DATE: Date = Date::constant(2018, 5, 25);

const ERASE_POLICY_ID: Uuid = uuid!("01639498-5000-7000-8000-000000000001");
const ERASE_RULE_ID: Uuid = uuid!("01639498-5000-7000-8000-000000000002");
const PSEUDONYMIZE_POLICY_ID: Uuid = uuid!("01639498-5000-7000-8000-000000000003");
const PSEUDONYMIZE_RULE_ID: Uuid = uuid!("01639498-5000-7000-8000-000000000004");

/// Build the GDPR Article 9 template for the picked treatment.
pub(crate) fn template(treatment: GdprArticle9Treatment) -> Template {
    match treatment {
        GdprArticle9Treatment::Erase => erase_template(),
        GdprArticle9Treatment::Pseudonymize => pseudonymize_template(),
    }
}

fn erase_template() -> Template {
    Template {
        id: "gdpr_article_9_erase".into(),
        name: "GDPR Article 9 special categories — erase".into(),
        version: Version::new(1, 0, 0),
        effective_date: EFFECTIVE_DATE,
        description: Some(
            "Erase the nine categories of personal data Article 9(1) treats as special.".into(),
        ),
        policy: erase_policy(),
    }
}

fn pseudonymize_template() -> Template {
    Template {
        id: "gdpr_article_9_pseudonymize".into(),
        name: "GDPR Article 9 special categories — pseudonymize".into(),
        version: Version::new(1, 0, 0),
        effective_date: EFFECTIVE_DATE,
        description: Some(
            "Pseudonymize the nine categories of personal data Article 9(1) treats as special. \
             Requires an Article 9(2) lawful-basis carve-out established out-of-band."
                .into(),
        ),
        policy: pseudonymize_policy(),
    }
}

fn erase_group() -> LabelGroup {
    LabelGroup {
        name: ERASE_GROUP.into(),
        description: Some(article_9_group_description().to_owned()),
        labels: GDPR_LABELS.to_vec(),
    }
}

fn pseudonymize_group() -> LabelGroup {
    LabelGroup {
        name: PSEUDONYMIZE_GROUP.into(),
        description: Some(article_9_group_description().to_owned()),
        labels: GDPR_LABELS.to_vec(),
    }
}

const fn article_9_group_description() -> &'static str {
    "The nine special categories of personal data enumerated in GDPR Article 9(1) \
     (racial/ethnic origin, political opinions, religious/philosophical beliefs, \
     trade-union membership, genetic data, biometric data for unique identification, \
     health data, sex life, sexual orientation)."
}

fn erase_policy() -> PolicyDefinition {
    PolicyDefinition {
        id: ERASE_POLICY_ID,
        name: "gdpr-article-9-erase".into(),
        description: Some(
            "Erase every Article 9(1) special-category entity by default. The posture for \
             callers without an Article 9(2) lawful-basis carve-out."
                .to_owned(),
        ),
        when: None,
        labels: Labels {
            builtins: GDPR_LABELS.to_vec(),
            custom: Vec::new(),
        },
        groups: vec![erase_group()],
        rules: vec![erase_rule()],
        fallback: None,
        retention: Vec::new(),
    }
}

fn pseudonymize_policy() -> PolicyDefinition {
    PolicyDefinition {
        id: PSEUDONYMIZE_POLICY_ID,
        name: "gdpr-article-9-pseudonymize".into(),
        description: Some(
            "Pseudonymize every Article 9(1) special-category entity (identity-preserving \
             surrogate). Requires an Article 9(2) lawful-basis carve-out established \
             out-of-band; the template does not verify or record the basis."
                .to_owned(),
        ),
        when: None,
        labels: Labels {
            builtins: GDPR_LABELS.to_vec(),
            custom: Vec::new(),
        },
        groups: vec![pseudonymize_group()],
        rules: vec![pseudonymize_rule()],
        fallback: None,
        retention: Vec::new(),
    }
}

fn erase_rule() -> PolicyRule {
    PolicyRule::Predicated(Box::new(PredicatedRule {
        id: ERASE_RULE_ID,
        name: "gdpr-article-9-erase".into(),
        description: Some(
            "Erase any entity whose label falls in the Article 9 special-category group."
                .to_owned(),
        ),
        predicate: Predicate::LabelInGroup {
            group: ERASE_GROUP.to_owned(),
        },
        action: ModalityRedactions::text(TextRedaction::Erase),
    }))
}

fn pseudonymize_rule() -> PolicyRule {
    PolicyRule::Predicated(Box::new(PredicatedRule {
        id: PSEUDONYMIZE_RULE_ID,
        name: "gdpr-article-9-pseudonymize".into(),
        description: Some(
            "Pseudonymize any entity whose label falls in the Article 9 special-category \
             group (identity-preserving surrogate)."
                .to_owned(),
        ),
        predicate: Predicate::LabelInGroup {
            group: PSEUDONYMIZE_GROUP.to_owned(),
        },
        action: ModalityRedactions::text(TextRedaction::Pseudonymize),
    }))
}

#[cfg(test)]
mod tests {
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
        let PolicyRule::Predicated(rule) = &template(GdprArticle9Treatment::Erase).policy.rules[0]
        else {
            panic!("expected Predicated rule");
        };
        assert!(matches!(rule.action.text, Some(TextRedaction::Erase)));
    }

    #[test]
    fn pseudonymize_treatment_uses_pseudonymize_action() {
        let PolicyRule::Predicated(rule) =
            &template(GdprArticle9Treatment::Pseudonymize).policy.rules[0]
        else {
            panic!("expected Predicated rule");
        };
        assert!(matches!(
            rule.action.text,
            Some(TextRedaction::Pseudonymize)
        ));
    }

    #[test]
    fn treatments_ship_distinct_policy_identities() {
        let e = template(GdprArticle9Treatment::Erase);
        let p = template(GdprArticle9Treatment::Pseudonymize);
        assert_ne!(e.id, p.id);
        assert_ne!(e.policy.id, p.policy.id);
    }

    #[test]
    fn template_ids_are_stable_across_calls() {
        for treatment in [
            GdprArticle9Treatment::Erase,
            GdprArticle9Treatment::Pseudonymize,
        ] {
            let a = template(treatment);
            let b = template(treatment);
            assert_eq!(a.id, b.id);
            assert_eq!(a.policy.id, b.policy.id);
            assert_eq!(a.policy.rules[0].id(), b.policy.rules[0].id());
        }
    }
}
