//! HIPAA Safe Harbor de-identification per 45 CFR §164.514(b)(2).
//!
//! Two rules under one policy, first-match-wins:
//!
//! 1. [`TableRule`] for the identifiers that need per-label
//!    dispatch — ages ≥ 90 [`Clamp`]ed to `"90 or older"`,
//!    dates related to an individual [`GeneralizeDate`]d to
//!    the year.
//! 2. [`Predicated`] rule keyed off the `hipaa_18`
//!    [`LabelGroup`] with plain [`Erase`] — catches every other
//!    identifier listed by the rule (names, contact info, IDs,
//!    biometrics, etc.).
//!
//! The [`TableRule`] fires first so `age` and date labels reach
//! their generalization operators; anything not in the table but
//! covered by the group falls through to rule 2. A caller who
//! wants a different terminal — [`Pseudonymize`] to keep
//! coreference across mentions, [`HmacHash`] with a per-tenant
//! key for a linkage-preserving audit — mutates the returned
//! [`PolicyDefinition`] before submitting.
//!
//! [`Clamp`]: nvisy_policy::redaction::TextRedaction::Clamp
//! [`Erase`]: nvisy_policy::redaction::TextRedaction::Erase
//! [`GeneralizeDate`]: nvisy_policy::redaction::TextRedaction::GeneralizeDate
//! [`HmacHash`]: nvisy_policy::redaction::TextRedaction::HmacHash
//! [`LabelGroup`]: nvisy_policy::LabelGroup
//! [`PolicyDefinition`]: nvisy_policy::PolicyDefinition
//! [`Predicated`]: nvisy_policy::PolicyRule::Predicated
//! [`Pseudonymize`]: nvisy_policy::redaction::TextRedaction::Pseudonymize
//! [`TableRule`]: nvisy_policy::TableRule

use elide_core::entity::LabelRef;
use jiff::civil::Date;
use nvisy_policy::predicate::Predicate;
use nvisy_policy::redaction::{
    ClampBucket, DateGranularity, DateStyle, ModalityRedactions, TextRedaction,
};
use nvisy_policy::{
    LabelEntry, LabelGroup, Labels, PolicyDefinition, PolicyRule, PredicatedRule, TableRule,
};
use semver::Version;
use uuid::{Uuid, uuid};

use super::Template;

/// Group name every HIPAA rule references.
pub(crate) const GROUP_NAME: &str = "hipaa_18";

/// Elide-builtin labels the group covers, one per HIPAA identifier
/// category. `date_of_birth` / `date_time` / `age` are
/// deliberately absent — they get per-label operators
/// ([`GeneralizeDate`] / [`Clamp`]) via the table rule, not the
/// bulk erase. Adding them here would collide with the table's
/// intent: first-match-wins currently protects the intent, but
/// leaving them out makes the split explicit and prevents a
/// future rule reorder from silently converting `age`→`Clamp`
/// into `age`→`Erase`.
///
/// [`Clamp`]: nvisy_policy::redaction::TextRedaction::Clamp
/// [`GeneralizeDate`]: nvisy_policy::redaction::TextRedaction::GeneralizeDate
const HIPAA_LABELS: &[LabelRef] = &[
    LabelRef::from_static("person_name"),
    LabelRef::from_static("address"),
    LabelRef::from_static("postal_code"),
    LabelRef::from_static("phone_number"),
    LabelRef::from_static("fax_number"),
    LabelRef::from_static("email_address"),
    LabelRef::from_static("government_id"),
    LabelRef::from_static("national_insurance_number"),
    LabelRef::from_static("medical_id"),
    LabelRef::from_static("insurance_id"),
    LabelRef::from_static("bank_account"),
    LabelRef::from_static("certificate_number"),
    LabelRef::from_static("drivers_license"),
    LabelRef::from_static("vehicle_id"),
    LabelRef::from_static("license_plate"),
    LabelRef::from_static("device_id"),
    LabelRef::from_static("url"),
    LabelRef::from_static("ip_address"),
    LabelRef::from_static("fingerprint"),
    LabelRef::from_static("voiceprint"),
    LabelRef::from_static("retina_scan"),
    LabelRef::from_static("facial_geometry"),
    LabelRef::from_static("genetic_data"),
    LabelRef::from_static("face"),
    LabelRef::from_static("internal_id"),
    LabelRef::from_static("case_number"),
];

/// Labels the table rule dispatches per-operator. Kept separate
/// from [`HIPAA_LABELS`] so the group's bulk-erase rule never
/// matches them — the table rule owns their operator dispatch.
const TABLE_LABELS: &[LabelRef] = &[
    LabelRef::from_static("age"),
    LabelRef::from_static("date_of_birth"),
    LabelRef::from_static("date_time"),
];

const POLICY_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000001");
const RULE_SPECIAL_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000002");
const RULE_BULK_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000003");

/// Build the HIPAA Safe Harbor template.
pub(crate) fn template() -> Template {
    Template {
        id: "hipaa_safe_harbor".into(),
        name: "HIPAA Safe Harbor de-identification".into(),
        version: Version::new(1, 0, 0),
        effective_date: Date::constant(2003, 4, 14),
        description: Some(
            "45 CFR §164.514(b)(2) — remove the eighteen identifier categories.".into(),
        ),
        policy: policy(),
    }
}

fn group() -> LabelGroup {
    LabelGroup {
        name: GROUP_NAME.into(),
        description: Some(
            "The 18 identifier categories the HIPAA Safe Harbor rule enumerates \
             (names, geographic subdivisions smaller than state, dates related to \
             an individual, contact info, government/medical/account/certificate \
             identifiers, vehicle and device ids, URLs, IPs, biometrics, faces, \
             and other unique identifiers)."
                .to_owned(),
        ),
        labels: HIPAA_LABELS.to_vec(),
    }
}

fn policy() -> PolicyDefinition {
    PolicyDefinition {
        id: POLICY_ID,
        name: "hipaa-safe-harbor".into(),
        description: Some(
            "HIPAA Safe Harbor de-identification. Ages ≥ 90 collapse to a bucket, \
             dates reduce to the year, every other identifier is erased."
                .to_owned(),
        ),
        when: None,
        labels: Labels {
            builtins: HIPAA_LABELS
                .iter()
                .chain(TABLE_LABELS.iter())
                .cloned()
                .collect(),
            custom: Vec::new(),
        },
        groups: vec![group()],
        rules: vec![special_dispatch_rule(), bulk_erase_rule()],
        fallback: None,
        retention: Vec::new(),
    }
}

/// §(C) ages > 89 collapse to `"90 or older"`; dates directly
/// related to an individual reduce to the year. Anything the
/// rule doesn't match falls through to the bulk erase.
fn special_dispatch_rule() -> PolicyRule {
    PolicyRule::Table(TableRule {
        id: RULE_SPECIAL_ID,
        name: "hipaa-age-and-dates".into(),
        description: Some(
            "§164.514(b)(2)(i)(C) — ages over 89 aggregate into a `90 or older` \
             bucket; dates related to the individual reduce to the year."
                .to_owned(),
        ),
        operators: vec![
            LabelEntry {
                label: LabelRef::from_static("age"),
                action: ModalityRedactions::text(TextRedaction::Clamp {
                    ceiling: Some(90.0),
                    ceiling_bucket: Some(ClampBucket::Plain("90 or older".to_owned())),
                    floor: None,
                    floor_bucket: None,
                    fallback: None,
                }),
            },
            LabelEntry {
                label: LabelRef::from_static("date_of_birth"),
                action: ModalityRedactions::text(TextRedaction::GeneralizeDate {
                    granularity: DateGranularity::Year,
                    style: DateStyle::Iso,
                    fallback: None,
                }),
            },
            LabelEntry {
                label: LabelRef::from_static("date_time"),
                action: ModalityRedactions::text(TextRedaction::GeneralizeDate {
                    granularity: DateGranularity::Year,
                    style: DateStyle::Iso,
                    fallback: None,
                }),
            },
        ],
    })
}

/// Everything else the group covers → [`Erase`].
///
/// [`Erase`]: nvisy_policy::redaction::TextRedaction::Erase
fn bulk_erase_rule() -> PolicyRule {
    PolicyRule::Predicated(Box::new(PredicatedRule {
        id: RULE_BULK_ID,
        name: "hipaa-bulk-erase".into(),
        description: Some("Every remaining §164.514(b)(2) identifier is erased.".to_owned()),
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
    fn table_labels_are_excluded_from_bulk_erase_group() {
        // Table-dispatched labels (age, dates) must not appear in
        // the bulk-erase group, else a rule reorder could silently
        // convert Clamp / GeneralizeDate into Erase.
        for label in TABLE_LABELS {
            assert!(
                !HIPAA_LABELS.contains(label),
                "table label `{}` must not appear in HIPAA_LABELS bulk-erase set",
                label.as_str(),
            );
        }
    }

    #[test]
    fn special_dispatch_rule_precedes_bulk_erase() {
        let policy = &template().policy;
        assert!(
            matches!(&policy.rules[0], PolicyRule::Table(_)),
            "table rule must fire first so `age` and dates reach their generalizers",
        );
        assert!(matches!(&policy.rules[1], PolicyRule::Predicated(_)));
    }

    #[test]
    fn table_rule_dispatches_age_to_clamp_and_dates_to_generalize() {
        let PolicyRule::Table(table) = &template().policy.rules[0] else {
            panic!("first rule must be Table");
        };
        let age = table
            .operators
            .iter()
            .find(|e| e.label.as_str() == "age")
            .expect("age entry present");
        assert!(matches!(
            age.action.text,
            Some(TextRedaction::Clamp { ceiling: Some(c), .. }) if (c - 90.0).abs() < f64::EPSILON,
        ));
        for date_label in ["date_of_birth", "date_time"] {
            let entry = table
                .operators
                .iter()
                .find(|e| e.label.as_str() == date_label)
                .unwrap_or_else(|| panic!("{date_label} entry present"));
            assert!(matches!(
                entry.action.text,
                Some(TextRedaction::GeneralizeDate { .. }),
            ));
        }
    }

    #[test]
    fn bulk_rule_matches_the_group() {
        let PolicyRule::Predicated(rule) = &template().policy.rules[1] else {
            panic!("second rule must be Predicated");
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
        assert_eq!(a.policy.id, b.policy.id);
        assert_eq!(a.policy.rules[0].id(), b.policy.rules[0].id());
        assert_eq!(a.policy.rules[1].id(), b.policy.rules[1].id());
    }
}
