//! GDPR Article 9 — special categories of personal data.
//!
//! Article 9(1) prohibits processing of nine categories of
//! personal data by default (racial/ethnic origin, political
//! opinions, religious/philosophical beliefs, trade-union
//! membership, genetic data, biometric data used to uniquely
//! identify a person, health data, sex life, sexual orientation).
//! The shipped template ships a single [`Predicated`] rule that
//! erases any entity whose label falls in the
//! `gdpr_article_9` [`LabelGroup`], leaving the caller to
//! override with [`Pseudonymize`] where a lawful-basis carve-out
//! (Article 9(2)) allows retention.
//!
//! [`LabelGroup`]: nvisy_policy::LabelGroup
//! [`Predicated`]: nvisy_policy::PolicyRule::Predicated
//! [`Pseudonymize`]: nvisy_policy::redaction::TextRedaction::Pseudonymize

use elide_core::entity::LabelRef;
use jiff::civil::Date;
use nvisy_policy::predicate::Predicate;
use nvisy_policy::redaction::{ModalityRedactions, TextRedaction};
use nvisy_policy::{LabelGroup, Labels, PolicyDefinition, PolicyRule, PredicatedRule};
use semver::Version;
use uuid::{Uuid, uuid};

use super::Template;

/// Group name every Article 9 rule references.
pub(crate) const GROUP_NAME: &str = "gdpr_article_9";

/// Elide-builtin labels the group covers, mapped from Article
/// 9(1) categories. See the caveats issue for two known
/// coverage gaps (`nationality` conflated with racial/ethnic
/// origin; no dedicated `sex_life` label).
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
    // Health data
    LabelRef::from_static("medical_id"),
    LabelRef::from_static("insurance_id"),
    LabelRef::from_static("prescription_id"),
    LabelRef::from_static("diagnosis"),
    LabelRef::from_static("medication"),
    // Sexual orientation (sex life has no dedicated label — see caveats)
    LabelRef::from_static("sexual_orientation"),
];

const POLICY_ID: Uuid = uuid!("01639498-5000-7000-8000-000000000001");
const RULE_ID: Uuid = uuid!("01639498-5000-7000-8000-000000000002");

/// Build the GDPR Article 9 template.
pub(crate) fn template() -> Template {
    Template {
        id: "gdpr_article_9".into(),
        name: "GDPR Article 9 special categories".into(),
        version: Version::new(1, 0, 0),
        effective_date: Date::constant(2018, 5, 25),
        description: Some(
            "Erase the nine categories of personal data Article 9(1) treats as special.".into(),
        ),
        policy: policy(),
    }
}

fn group() -> LabelGroup {
    LabelGroup {
        name: GROUP_NAME.into(),
        description: Some(
            "The nine special categories of personal data enumerated in \
             GDPR Article 9(1) (racial/ethnic origin, political opinions, \
             religious/philosophical beliefs, trade-union membership, genetic \
             data, biometric data for unique identification, health data, sex \
             life, sexual orientation)."
                .to_owned(),
        ),
        labels: GDPR_LABELS.to_vec(),
    }
}

fn policy() -> PolicyDefinition {
    PolicyDefinition {
        id: POLICY_ID,
        name: "gdpr-article-9".into(),
        description: Some(
            "Erase every Article 9(1) special-category entity by default. \
             Callers with an Article 9(2) lawful-basis carve-out override \
             the operator to Pseudonymize or HmacHash on the returned rule."
                .to_owned(),
        ),
        when: None,
        labels: Labels {
            builtins: GDPR_LABELS.to_vec(),
            custom: Vec::new(),
        },
        groups: vec![group()],
        rules: vec![erase_rule()],
        fallback: None,
        retention: Vec::new(),
    }
}

fn erase_rule() -> PolicyRule {
    PolicyRule::Predicated(Box::new(PredicatedRule {
        id: RULE_ID,
        name: "gdpr-article-9-erase".into(),
        description: Some(
            "Erase any entity whose label falls in the gdpr_article_9 group.".to_owned(),
        ),
        predicate: Predicate::LabelInGroup {
            group: GROUP_NAME.to_owned(),
        },
        action: ModalityRedactions::text(TextRedaction::Erase),
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
            "sexual_orientation",
        ] {
            assert!(
                GDPR_LABELS.iter().any(|l| l.as_str() == anchor),
                "expected anchor label `{anchor}` in gdpr_article_9 group",
            );
        }
    }

    #[test]
    fn template_ids_are_stable_across_calls() {
        let a = template();
        let b = template();
        assert_eq!(a.policy.id, b.policy.id);
        assert_eq!(a.policy.rules[0].id(), b.policy.rules[0].id());
    }
}
