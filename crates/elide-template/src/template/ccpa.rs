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
//! [`LabelGroup`]: elide_governance::LabelGroup
//! [`PolicyDefinition`]: elide_governance::PolicyDefinition
//! [`Predicated`]: elide_governance::RuleDispatch::Predicated
//! [`Pseudonymize`]: elide_governance::redaction::TextRedaction::Pseudonymize

use elide_core::entity::LabelRef;
use elide_governance::predicate::Predicate;
use elide_governance::redaction::{ModalityRedactions, TextRedaction};
use elide_governance::{LabelGroup, Labels, PolicyDefinition, PolicyRule, RuleDispatch};
use jiff::civil::Date;
use semver::Version;
use uuid::{Uuid, uuid};

use super::Template;

/// Group name every CCPA rule references.
pub(crate) const GROUP_NAME: &str = "ccpa_personal_information";

/// Elide-builtin labels the group covers, mapped from
/// §1798.140(v)(1) categories.
const CCPA_LABELS: &[LabelRef] = &[
    // (A) Identifiers — includes geographic subdivisions (finer
    // than country / state), account credentials, and dates
    // directly related to an individual.
    LabelRef::from_static("person_name"),
    LabelRef::from_static("address"),
    LabelRef::from_static("street_address"),
    LabelRef::from_static("city"),
    LabelRef::from_static("postal_code"),
    LabelRef::from_static("email_address"),
    LabelRef::from_static("phone_number"),
    LabelRef::from_static("ip_address"),
    LabelRef::from_static("mac_address"),
    LabelRef::from_static("device_id"),
    LabelRef::from_static("username"),
    LabelRef::from_static("password"),
    LabelRef::from_static("security_question_answer"),
    LabelRef::from_static("government_id"),
    LabelRef::from_static("national_insurance_number"),
    LabelRef::from_static("drivers_license"),
    LabelRef::from_static("passport_number"),
    LabelRef::from_static("individual_date"),
    // (B) §1798.80 categories overlap with (A) + adds signature,
    // physical description, education/employment, financial /
    // medical / insurance. `health_narrative` catches the
    // free-form clinical content that specific IDs miss.
    LabelRef::from_static("signature"),
    LabelRef::from_static("handwriting"),
    LabelRef::from_static("occupation"),
    LabelRef::from_static("medical_id"),
    LabelRef::from_static("insurance_id"),
    LabelRef::from_static("health_narrative"),
    LabelRef::from_static("bank_account"),
    // (C) Protected classifications (CA/federal)
    LabelRef::from_static("ethnicity"),
    LabelRef::from_static("nationality"),
    LabelRef::from_static("religion"),
    LabelRef::from_static("gender"),
    LabelRef::from_static("sexual_orientation"),
    LabelRef::from_static("sex_life"),
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
    // (F) Internet / network activity — includes CPRA §1798.140(ae)(4)
    // communications content (mail/email/text/chat bodies).
    LabelRef::from_static("url"),
    LabelRef::from_static("communications_content"),
    // (G) Geolocation data — CPRA §1798.140(ae)(2) singles out
    // precise geolocation (≤1850 ft) as SPI; both survive erase
    // under regular PI.
    LabelRef::from_static("coordinates"),
    LabelRef::from_static("geolocation_metadata"),
    LabelRef::from_static("precise_geolocation"),
    // (H) Sensory (audio, visual, thermal, olfactory)
    LabelRef::from_static("face"),
    // (I) Professional / employment information
    LabelRef::from_static("certificate_number"),
    LabelRef::from_static("company_id"),
    LabelRef::from_static("department_name"),
    // (J) Non-public education information per FERPA
    LabelRef::from_static("education_record"),
    // (K) Inferences drawn to build a consumer profile
    LabelRef::from_static("inference"),
];

const POLICY_ID: Uuid = uuid!("016806b5-bc00-7000-8000-000000000001");
const RULE_ID: Uuid = uuid!("016806b5-bc00-7000-8000-000000000002");

/// Build the CCPA template.
pub(crate) fn template() -> Template {
    Template {
        id: "ccpa".into(),
        name: "CCPA / CPRA personal information".into(),
        version: Version::new(1, 0, 0),
        effective_date: Date::constant(2020, 1, 1),
        description: Some(
            "Cal. Civ. Code §1798.140(v)(1) — erase every enumerated personal information \
             category."
                .into(),
        ),
        policy: policy(),
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
                .into(),
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
                .into(),
        ),
        labels: Labels {
            builtins: CCPA_LABELS.to_vec(),
            custom: Vec::new(),
        },
        groups: vec![group()],
        rules: vec![erase_rule()],
        fallback: None,
    }
}

fn erase_rule() -> PolicyRule {
    PolicyRule {
        id: RULE_ID,
        name: "ccpa-personal-information-erase".into(),
        description: Some(
            "Erase any entity whose label falls in the ccpa_personal_information \
             group."
                .into(),
        ),
        dispatch: RuleDispatch::Predicated {
            predicate: Predicate::LabelInGroup {
                group: GROUP_NAME.to_owned(),
            },
            action: Box::new(ModalityRedactions::text(TextRedaction::Erase)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_ccpa_category_has_at_least_one_anchor_label() {
        // Spot-check one label per shipped category so a future
        // edit dropping a whole category trips this rather than
        // silently regressing coverage.
        for anchor in [
            "person_name",      // (A) identifiers
            "signature",        // (B) §1798.80
            "ethnicity",        // (C) protected classifications
            "payment_card",     // (D) commercial info
            "fingerprint",      // (E) biometric
            "url",              // (F) internet/network activity
            "coordinates",      // (G) geolocation
            "face",             // (H) sensory
            "occupation",       // (I) professional/employment
            "education_record", // (J) non-public education info
            "inference",        // (K) inferences
        ] {
            assert!(
                CCPA_LABELS.iter().any(|l| l.as_str() == anchor),
                "expected anchor label `{anchor}` in ccpa_personal_information group",
            );
        }
    }

    #[test]
    fn template_ids_are_stable_across_calls() {
        let a = template();
        let b = template();
        assert_eq!(a.policy.id, b.policy.id);
        assert_eq!(a.policy.rules[0].id, b.policy.rules[0].id);
    }
}
