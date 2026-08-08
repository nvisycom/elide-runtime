//! CCPA / CPRA — "personal information" categories per
//! Cal. Civ. Code §1798.140(v)(1).
//!
//! The statute enumerates eleven categories of personal
//! information (subsections (A)–(K)). The shipped template
//! rolls the elide-builtin labels that map onto those categories
//! into a `ccpa_personal_information` [`LabelGroup`], and ships
//! a single [`Predicated`] rule that erases every match.
//!
//! Consumer requests under CCPA fall into two large buckets:
//! disclosure (right to know) and deletion (right to delete). The
//! shipped template targets the deletion posture — a workflow
//! that surfaces PI and hands the caller a redacted copy on
//! request. Callers whose posture retains PI under a
//! §1798.145 exception (fraud detection, security incident,
//! transactional necessity, ...) override the operator on the
//! returned [`PolicyDefinition`] — commonly to [`Pseudonymize`]
//! for retained analytics that keep coreference without exposing
//! the underlying identifier.
//!
//! [`LabelGroup`]: nvisy_policy::LabelGroup
//! [`PolicyDefinition`]: nvisy_policy::PolicyDefinition
//! [`Predicated`]: nvisy_policy::PolicyRule::Predicated
//! [`Pseudonymize`]: nvisy_policy::redaction::TextRedaction::Pseudonymize

use elide_core::entity::LabelRef;
use jiff::civil::Date;
use nvisy_policy::predicate::Predicate;
use nvisy_policy::redaction::TextRedaction;
use nvisy_policy::{LabelGroup, Labels, PolicyDefinition, PolicyRule, PredicatedRule};
use semver::Version;
use uuid::{Uuid, uuid};

use super::{Template, text_action};

/// Group name every CCPA rule references.
pub(crate) const GROUP_NAME: &str = "ccpa_personal_information";

/// Elide-builtin labels the group covers, mapped from
/// §1798.140(v)(1) categories. See the caveats issue for two
/// known coverage gaps ((J) non-public education info, (K)
/// inferences — neither has a dedicated label today).
const CCPA_LABELS: &[LabelRef] = &[
    // (A) Identifiers
    LabelRef::from_static("person_name"),
    LabelRef::from_static("address"),
    LabelRef::from_static("postal_code"),
    LabelRef::from_static("email_address"),
    LabelRef::from_static("phone_number"),
    LabelRef::from_static("ip_address"),
    LabelRef::from_static("mac_address"),
    LabelRef::from_static("device_id"),
    LabelRef::from_static("username"),
    LabelRef::from_static("government_id"),
    LabelRef::from_static("national_insurance_number"),
    LabelRef::from_static("drivers_license"),
    LabelRef::from_static("passport_number"),
    // (B) §1798.80 categories overlap with (A) + adds signature,
    // physical description, education/employment, financial /
    // medical / insurance.
    LabelRef::from_static("signature"),
    LabelRef::from_static("handwriting"),
    LabelRef::from_static("occupation"),
    LabelRef::from_static("medical_id"),
    LabelRef::from_static("insurance_id"),
    LabelRef::from_static("bank_account"),
    // (C) Protected classifications (CA/federal)
    LabelRef::from_static("ethnicity"),
    LabelRef::from_static("nationality"),
    LabelRef::from_static("religion"),
    LabelRef::from_static("gender"),
    LabelRef::from_static("sexual_orientation"),
    LabelRef::from_static("age"),
    LabelRef::from_static("date_of_birth"),
    // (D) Commercial information
    LabelRef::from_static("payment_card"),
    LabelRef::from_static("amount"),
    // (E) Biometric information
    LabelRef::from_static("fingerprint"),
    LabelRef::from_static("voiceprint"),
    LabelRef::from_static("retina_scan"),
    LabelRef::from_static("facial_geometry"),
    LabelRef::from_static("genetic_data"),
    // (F) Internet / network activity
    LabelRef::from_static("url"),
    // (G) Geolocation data
    LabelRef::from_static("coordinates"),
    LabelRef::from_static("geolocation_metadata"),
    // (H) Sensory (audio, visual, thermal, olfactory)
    LabelRef::from_static("face"),
    // (I) Professional / employment information
    LabelRef::from_static("certificate_number"),
    LabelRef::from_static("company_id"),
    LabelRef::from_static("department_name"),
];

const POLICY_ID: Uuid = uuid!("016806b5-bc00-7000-8000-000000000001");
const RULE_ID: Uuid = uuid!("016806b5-bc00-7000-8000-000000000002");

/// Build the CCPA template.
pub(crate) fn template() -> Template {
    Template {
        name: "ccpa".into(),
        version: Version::new(1, 0, 0),
        effective_date: Date::constant(2020, 1, 1),
        description: "CCPA / CPRA — Cal. Civ. Code §1798.140(v)(1) personal information".into(),
        groups: vec![group()],
        policies: vec![policy()],
    }
}

fn group() -> LabelGroup {
    LabelGroup {
        name: GROUP_NAME.into(),
        description: Some(
            "The eleven personal-information categories enumerated in \
             Cal. Civ. Code §1798.140(v)(1) (identifiers, §1798.80 categories, \
             protected classifications, commercial information, biometric data, \
             internet/network activity, geolocation, sensory data, professional/\
             employment information, non-public education info, inferences)."
                .to_owned(),
        ),
        labels: CCPA_LABELS.to_vec(),
    }
}

fn policy() -> PolicyDefinition {
    PolicyDefinition {
        id: POLICY_ID,
        name: "ccpa-personal-information".into(),
        description: Some(
            "Erase every §1798.140(v)(1) personal-information entity by default. \
             Callers under a §1798.145 retention exception override the operator \
             to Pseudonymize on the returned rule."
                .to_owned(),
        ),
        when: None,
        labels: Labels {
            builtins: CCPA_LABELS.to_vec(),
            custom: Vec::new(),
        },
        rules: vec![erase_rule()],
        fallback: None,
        retention: Vec::new(),
    }
}

fn erase_rule() -> PolicyRule {
    PolicyRule::Predicated(Box::new(PredicatedRule {
        id: RULE_ID,
        name: "ccpa-personal-information-erase".into(),
        description: Some(
            "Erase any entity whose label falls in the ccpa_personal_information \
             group."
                .to_owned(),
        ),
        predicate: Predicate::LabelInGroup {
            group: GROUP_NAME.to_owned(),
        },
        action: text_action(TextRedaction::Erase),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_carries_group_and_single_policy() {
        let t = template();
        assert_eq!(t.name, "ccpa");
        assert_eq!(t.version, Version::new(1, 0, 0));
        assert_eq!(t.groups.len(), 1);
        assert_eq!(t.groups[0].name, GROUP_NAME);
        assert_eq!(t.policies.len(), 1);
    }

    #[test]
    fn every_shipped_ccpa_category_has_at_least_one_anchor_label() {
        // Spot-check one label per shipped category so a future
        // edit dropping a whole category trips this rather than
        // silently regressing coverage. Categories (J) and (K)
        // aren't covered — see caveats issue.
        for anchor in [
            "person_name",  // (A) identifiers
            "signature",    // (B) §1798.80
            "ethnicity",    // (C) protected classifications
            "payment_card", // (D) commercial info
            "fingerprint",  // (E) biometric
            "url",          // (F) internet/network activity
            "coordinates",  // (G) geolocation
            "face",         // (H) sensory
            "occupation",   // (I) professional/employment
        ] {
            assert!(
                CCPA_LABELS.iter().any(|l| l.as_str() == anchor),
                "expected anchor label `{anchor}` in ccpa_personal_information group",
            );
        }
    }

    #[test]
    fn rule_matches_the_group_and_erases() {
        let PolicyRule::Predicated(rule) = &template().policies[0].rules[0] else {
            panic!("expected Predicated rule");
        };
        assert!(matches!(
            &rule.predicate,
            Predicate::LabelInGroup { group } if group == GROUP_NAME,
        ));
        assert!(matches!(rule.action.text, Some(TextRedaction::Erase)));
    }

    #[test]
    fn template_ids_are_stable_across_calls() {
        let a = template();
        let b = template();
        assert_eq!(a.policies[0].id, b.policies[0].id);
        assert_eq!(a.policies[0].rules[0].id(), b.policies[0].rules[0].id());
    }
}
