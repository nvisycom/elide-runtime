//! HIPAA §164.514 de-identification.
//!
//! §164.514(b) offers two paths to de-identification: **Safe
//! Harbor** (a fixed rule set — remove eighteen identifier
//! categories with age/date special dispatch) and **Expert
//! Determination** (a statistician-signed attestation of "very
//! small" re-identification risk). §164.514(e) then defines the
//! **Limited Data Set** — a narrower subtraction that keeps
//! dates and coarse geography for research handoffs governed by
//! a Data Use Agreement.
//!
//! This module ships all three:
//!
//! - [`HipaaDeidMethod::SafeHarbor`] — every §164.514(b)(2)
//!   identifier removed. Ages ≥ 90 [`Clamp`]ed to `"90 or older"`,
//!   dates related to an individual [`GeneralizeDate`]d to the
//!   year, the remainder [`Erase`]d.
//! - [`HipaaDeidMethod::LimitedDataSet`] — §164.514(e)(2)'s
//!   sixteen-identifier subtraction. Names, street address,
//!   contact info, IDs, biometrics, and other unique identifiers
//!   erase; dates, town/city, state, ZIP, and ages survive
//!   verbatim (the DUA is the caller's out-of-band obligation).
//! - [`HipaaDeidMethod::ExpertDetermination`] — a starting
//!   scaffold for §164.514(b)(1). Same 18-identifier label set
//!   as Safe Harbor, same age/date table dispatch, but the bulk
//!   terminal is [`Pseudonymize`] instead of [`Erase`] to
//!   preserve coreference across mentions (the analytics need
//!   that most Expert Determination processes target). Does not
//!   certify de-identification; a qualified statistician must
//!   still document that re-identification risk is "very small"
//!   under the applicable methodology.
//!
//! [`Clamp`]: nvisy_policy::redaction::TextRedaction::Clamp
//! [`Erase`]: nvisy_policy::redaction::TextRedaction::Erase
//! [`GeneralizeDate`]: nvisy_policy::redaction::TextRedaction::GeneralizeDate
//! [`HmacHash`]: nvisy_policy::redaction::TextRedaction::HmacHash
//! [`LabelGroup`]: nvisy_policy::LabelGroup
//! [`PolicyDefinition`]: nvisy_policy::PolicyDefinition
//! [`Predicated`]: nvisy_policy::RuleDispatch::Predicated
//! [`Pseudonymize`]: nvisy_policy::redaction::TextRedaction::Pseudonymize
//! [`RuleDispatch`]: nvisy_policy::RuleDispatch

use elide_core::entity::LabelRef;
use jiff::civil::Date;
use nvisy_policy::predicate::Predicate;
use nvisy_policy::redaction::{
    ClampBucket, DateGranularity, DateStyle, ModalityRedactions, TextRedaction,
};
use nvisy_policy::{LabelEntry, LabelGroup, Labels, PolicyDefinition, PolicyRule, RuleDispatch};
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::{Uuid, uuid};

use super::Template;

/// Which HIPAA §164.514 de-identification method to apply.
///
/// The tradeoff is analytic yield vs. downstream constraint:
///
/// - [`SafeHarbor`](Self::SafeHarbor) — strips all eighteen
///   identifier categories. No Data Use Agreement or statistician
///   required, but dates, coarse geography, and ages ≥ 90 all
///   disappear or collapse.
/// - [`LimitedDataSet`](Self::LimitedDataSet) — narrower
///   subtraction (§164.514(e)(2)) that keeps dates, town/city,
///   state, ZIP, and ages verbatim. Suitable for research and
///   public-health handoffs *only when* a Data Use Agreement
///   governs the recipient's use.
/// - [`ExpertDetermination`](Self::ExpertDetermination) —
///   starting scaffold for §164.514(b)(1). Same label set as
///   Safe Harbor with pseudonymization as the default terminal
///   (identity-preserving across mentions). **Does not certify
///   de-identification** — a qualified statistician must
///   document that re-identification risk is "very small" under
///   the applicable methodology before the output can be treated
///   as de-identified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HipaaDeidMethod {
    /// Safe Harbor de-identification per §164.514(b)(2).
    SafeHarbor,
    /// Limited Data Set per §164.514(e)(2). Requires a Data Use
    /// Agreement out-of-band.
    LimitedDataSet,
    /// Expert Determination scaffold for §164.514(b)(1). Requires
    /// a qualified statistician to document that re-identification
    /// risk is "very small" — the shipped template alone does not
    /// certify de-identification.
    ExpertDetermination,
}

/// Group name Safe Harbor's bulk-erase rule references.
const SAFE_HARBOR_GROUP: &str = "hipaa_safe_harbor";
/// Group name Limited Data Set's bulk-erase rule references.
const LDS_GROUP: &str = "hipaa_limited_data_set";
/// Group name Expert Determination's bulk-pseudonymize rule
/// references. Same label membership as Safe Harbor; the
/// separate group name lets audits distinguish the two postures
/// by the group id alone.
const ED_GROUP: &str = "hipaa_expert_determination";

/// Every label the Safe Harbor bulk-erase rule targets.
/// `age`, `date_of_birth`, `individual_date` are absent — the
/// table rule owns them.
const SAFE_HARBOR_LABELS: &[LabelRef] = &[
    LabelRef::from_static("person_name"),
    // §(B) geographic subdivisions smaller than state — every
    // level from `address` blob down to `city` erases; `state`
    // and `country` are permitted to survive per Safe Harbor and
    // are deliberately absent.
    LabelRef::from_static("address"),
    LabelRef::from_static("street_address"),
    LabelRef::from_static("city"),
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
    // §(R) "any other unique identifying number, characteristic,
    // or code" — the elide-builtin catch-all catches ad-hoc
    // identifiers (badge numbers, room numbers, provider
    // taxonomy codes) that don't map to a specific label.
    LabelRef::from_static("unresolved"),
];

/// Labels Safe Harbor's table rule dispatches per-operator.
/// Kept separate from [`SAFE_HARBOR_LABELS`] so the bulk-erase
/// rule never matches them.
///
/// `date_time` deliberately absent: §(C) targets dates *directly
/// related to an individual*; generic `date_time` (invoice
/// dates, meeting timestamps) shouldn't be generalized. Elide's
/// `individual_date` label is the narrower fit.
const SAFE_HARBOR_TABLE_LABELS: &[LabelRef] = &[
    LabelRef::from_static("age"),
    LabelRef::from_static("date_of_birth"),
    LabelRef::from_static("individual_date"),
];

/// Every label the Limited Data Set bulk-erase rule targets.
/// Sixteen categories per §164.514(e)(2) — dates, ages,
/// town/city, state, and ZIP survive (dropped from this list vs.
/// Safe Harbor's). Only `street_address` erases from the
/// geographic set; if the recognizer emits the broader `address`
/// blob rather than the fine-grained split, the LDS survivor
/// intent is defeated for that entity — enable elide's
/// address-split patterns to close that gap.
const LDS_LABELS: &[LabelRef] = &[
    LabelRef::from_static("person_name"),
    LabelRef::from_static("street_address"),
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
    // §164.514(e)(2)(xvi) "any other unique identifying number,
    // characteristic, or code" — catch-all for ad-hoc identifiers
    // that don't map to a specific label.
    LabelRef::from_static("unresolved"),
];

const SAFE_HARBOR_POLICY_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000001");
const SAFE_HARBOR_TABLE_RULE_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000002");
const SAFE_HARBOR_BULK_RULE_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000003");
const LDS_POLICY_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000004");
const LDS_BULK_RULE_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000005");
const ED_POLICY_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000006");
const ED_TABLE_RULE_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000007");
const ED_BULK_RULE_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000008");

/// §164.514 effective date (final Privacy Rule).
const EFFECTIVE_DATE: Date = Date::constant(2003, 4, 14);

/// Build the HIPAA §164.514 template for the picked method.
pub(crate) fn template(method: HipaaDeidMethod) -> Template {
    match method {
        HipaaDeidMethod::SafeHarbor => safe_harbor_template(),
        HipaaDeidMethod::LimitedDataSet => limited_data_set_template(),
        HipaaDeidMethod::ExpertDetermination => expert_determination_template(),
    }
}

fn safe_harbor_template() -> Template {
    Template {
        id: "hipaa_deid_safe_harbor".into(),
        name: "HIPAA §164.514(b)(2) Safe Harbor".into(),
        version: Version::new(1, 0, 0),
        effective_date: EFFECTIVE_DATE,
        description: Some(
            "Remove the eighteen identifier categories the Safe Harbor rule enumerates.".into(),
        ),
        policy: safe_harbor_policy(),
    }
}

fn limited_data_set_template() -> Template {
    Template {
        id: "hipaa_deid_limited_data_set".into(),
        name: "HIPAA §164.514(e)(2) Limited Data Set".into(),
        version: Version::new(1, 0, 0),
        effective_date: EFFECTIVE_DATE,
        description: Some(
            "Remove the sixteen identifier categories §164.514(e)(2) enumerates. \
             Requires a Data Use Agreement out-of-band."
                .into(),
        ),
        policy: limited_data_set_policy(),
    }
}

fn expert_determination_template() -> Template {
    Template {
        id: "hipaa_deid_expert_determination".into(),
        name: "HIPAA §164.514(b)(1) Expert Determination (scaffold)".into(),
        version: Version::new(1, 0, 0),
        effective_date: EFFECTIVE_DATE,
        description: Some(
            "Starting scaffold for HIPAA §164.514(b)(1) Expert Determination. \
             Ships the 18-identifier label set with pseudonymization as the \
             default terminal (identity-preserving across mentions). Does not \
             certify de-identification. §164.514(b)(1) requires a qualified \
             statistician to apply generally-accepted principles and methods, \
             determine that re-identification risk is `very small`, and document \
             that determination. Shipping this template's output as \
             de-identified without that documentation violates the rule."
                .into(),
        ),
        policy: expert_determination_policy(),
    }
}

fn safe_harbor_group() -> LabelGroup {
    LabelGroup {
        name: SAFE_HARBOR_GROUP.into(),
        description: Some(
            "The 18 identifier categories the HIPAA Safe Harbor rule enumerates \
             (names, geographic subdivisions smaller than state, dates related to \
             an individual, contact info, government/medical/account/certificate \
             identifiers, vehicle and device ids, URLs, IPs, biometrics, faces, \
             and other unique identifiers)."
                .into(),
        ),
        labels: SAFE_HARBOR_LABELS.to_vec(),
    }
}

fn lds_group() -> LabelGroup {
    LabelGroup {
        name: LDS_GROUP.into(),
        description: Some(
            "The 16 identifier categories §164.514(e)(2) enumerates for the \
             Limited Data Set. Dates, ages, town/city, state, and ZIP survive \
             verbatim under this posture; a Data Use Agreement governs the \
             recipient's use out-of-band."
                .into(),
        ),
        labels: LDS_LABELS.to_vec(),
    }
}

fn ed_group() -> LabelGroup {
    LabelGroup {
        name: ED_GROUP.into(),
        description: Some(
            "The 18 identifier categories carried into the Expert Determination \
             scaffold. Same label membership as Safe Harbor; the separate group \
             name lets audits distinguish the two postures by group id alone."
                .into(),
        ),
        labels: SAFE_HARBOR_LABELS.to_vec(),
    }
}

fn safe_harbor_policy() -> PolicyDefinition {
    PolicyDefinition {
        id: SAFE_HARBOR_POLICY_ID,
        name: "hipaa-safe-harbor".into(),
        description: Some(
            "HIPAA Safe Harbor de-identification. Ages ≥ 90 collapse to a bucket, \
             dates reduce to the year, every other identifier is erased."
                .into(),
        ),
        labels: Labels {
            builtins: SAFE_HARBOR_LABELS
                .iter()
                .chain(SAFE_HARBOR_TABLE_LABELS.iter())
                .cloned()
                .collect(),
            custom: Vec::new(),
        },
        groups: vec![safe_harbor_group()],
        rules: vec![safe_harbor_table_rule(), safe_harbor_bulk_erase_rule()],
        fallback: None,
        retention: Vec::new(),
    }
}

fn limited_data_set_policy() -> PolicyDefinition {
    PolicyDefinition {
        id: LDS_POLICY_ID,
        name: "hipaa-limited-data-set".into(),
        description: Some(
            "HIPAA Limited Data Set. Sixteen identifier categories erase; dates, \
             ages, town/city, state, and ZIP survive verbatim."
                .into(),
        ),
        labels: Labels {
            builtins: LDS_LABELS.to_vec(),
            custom: Vec::new(),
        },
        groups: vec![lds_group()],
        rules: vec![lds_bulk_erase_rule()],
        fallback: None,
        retention: Vec::new(),
    }
}

fn expert_determination_policy() -> PolicyDefinition {
    PolicyDefinition {
        id: ED_POLICY_ID,
        name: "hipaa-expert-determination".into(),
        description: Some(
            "HIPAA Expert Determination scaffold. Same 18-identifier label set \
             as Safe Harbor with pseudonymization as the default terminal to \
             preserve coreference across mentions. A qualified statistician \
             must attest that re-identification risk is `very small` for the \
             recipient's dataset before the output can be treated as \
             de-identified."
                .into(),
        ),
        labels: Labels {
            builtins: SAFE_HARBOR_LABELS
                .iter()
                .chain(SAFE_HARBOR_TABLE_LABELS.iter())
                .cloned()
                .collect(),
            custom: Vec::new(),
        },
        groups: vec![ed_group()],
        rules: vec![ed_table_rule(), ed_bulk_pseudonymize_rule()],
        fallback: None,
        retention: Vec::new(),
    }
}

/// §(C) ages > 89 collapse to `"90 or older"`; dates directly
/// related to an individual reduce to the year. Anything the
/// rule doesn't match falls through to the bulk erase.
fn safe_harbor_table_rule() -> PolicyRule {
    PolicyRule {
        id: SAFE_HARBOR_TABLE_RULE_ID,
        name: "hipaa-age-and-dates".into(),
        description: Some(
            "§164.514(b)(2)(i)(C) — ages over 89 aggregate into a `90 or older` \
             bucket; dates related to the individual reduce to the year."
                .into(),
        ),
        dispatch: RuleDispatch::Table {
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
                    label: LabelRef::from_static("individual_date"),
                    action: ModalityRedactions::text(TextRedaction::GeneralizeDate {
                        granularity: DateGranularity::Year,
                        style: DateStyle::Iso,
                        fallback: None,
                    }),
                },
            ],
        },
    }
}

/// Everything the Safe Harbor group covers → [`Erase`].
///
/// [`Erase`]: nvisy_policy::redaction::TextRedaction::Erase
fn safe_harbor_bulk_erase_rule() -> PolicyRule {
    PolicyRule {
        id: SAFE_HARBOR_BULK_RULE_ID,
        name: "hipaa-safe-harbor-bulk-erase".into(),
        description: Some("Every remaining §164.514(b)(2) identifier is erased.".into()),
        dispatch: RuleDispatch::Predicated {
            predicate: Predicate::LabelInGroup {
                group: SAFE_HARBOR_GROUP.to_owned(),
            },
            action: Box::new(ModalityRedactions::text(TextRedaction::Erase)),
        },
    }
}

/// Everything the Limited Data Set group covers → [`Erase`].
///
/// [`Erase`]: nvisy_policy::redaction::TextRedaction::Erase
fn lds_bulk_erase_rule() -> PolicyRule {
    PolicyRule {
        id: LDS_BULK_RULE_ID,
        name: "hipaa-lds-bulk-erase".into(),
        description: Some("Every §164.514(e)(2) identifier is erased.".into()),
        dispatch: RuleDispatch::Predicated {
            predicate: Predicate::LabelInGroup {
                group: LDS_GROUP.to_owned(),
            },
            action: Box::new(ModalityRedactions::text(TextRedaction::Erase)),
        },
    }
}

/// Same table dispatch as Safe Harbor — ages ≥ 90 collapse to
/// the bucket, dates reduce to the year — carried into the
/// Expert Determination scaffold as sensible defaults the
/// statistician can override.
fn ed_table_rule() -> PolicyRule {
    PolicyRule {
        id: ED_TABLE_RULE_ID,
        name: "hipaa-ed-age-and-dates".into(),
        description: Some(
            "Age/date dispatch carried into the Expert Determination scaffold. \
             The statistician's risk analysis may override these defaults."
                .into(),
        ),
        dispatch: RuleDispatch::Table {
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
                    label: LabelRef::from_static("individual_date"),
                    action: ModalityRedactions::text(TextRedaction::GeneralizeDate {
                        granularity: DateGranularity::Year,
                        style: DateStyle::Iso,
                        fallback: None,
                    }),
                },
            ],
        },
    }
}

/// Everything the Expert Determination group covers →
/// [`Pseudonymize`]. Preserves coreference across mentions so
/// downstream analytics can still join on the surrogate.
///
/// [`Pseudonymize`]: nvisy_policy::redaction::TextRedaction::Pseudonymize
fn ed_bulk_pseudonymize_rule() -> PolicyRule {
    PolicyRule {
        id: ED_BULK_RULE_ID,
        name: "hipaa-ed-bulk-pseudonymize".into(),
        description: Some(
            "Every identifier in the Expert Determination label set is \
             pseudonymized (identity-preserving surrogate). Statistician may \
             override to Erase / HmacHash / Encrypt as their risk analysis \
             demands."
                .into(),
        ),
        dispatch: RuleDispatch::Predicated {
            predicate: Predicate::LabelInGroup {
                group: ED_GROUP.to_owned(),
            },
            action: Box::new(ModalityRedactions::text(TextRedaction::Pseudonymize)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_harbor_table_labels_are_excluded_from_bulk_erase_group() {
        // Table-dispatched labels (age, dates) must not appear in
        // the bulk-erase group, else a rule reorder could silently
        // convert Clamp / GeneralizeDate into Erase.
        for label in SAFE_HARBOR_TABLE_LABELS {
            assert!(
                !SAFE_HARBOR_LABELS.contains(label),
                "table label `{}` must not appear in SAFE_HARBOR_LABELS bulk-erase set",
                label.as_str(),
            );
        }
    }

    #[test]
    fn safe_harbor_special_dispatch_rule_precedes_bulk_erase() {
        let policy = &template(HipaaDeidMethod::SafeHarbor).policy;
        assert!(
            matches!(&policy.rules[0].dispatch, RuleDispatch::Table { .. }),
            "table rule must fire first so `age` and dates reach their generalizers",
        );
        assert!(matches!(
            &policy.rules[1].dispatch,
            RuleDispatch::Predicated { .. }
        ));
    }

    #[test]
    fn safe_harbor_table_rule_dispatches_age_to_clamp_and_dates_to_generalize() {
        let RuleDispatch::Table { operators } =
            &template(HipaaDeidMethod::SafeHarbor).policy.rules[0].dispatch
        else {
            panic!("first rule must be Table dispatch");
        };
        let age = operators
            .iter()
            .find(|e| e.label.as_str() == "age")
            .expect("age entry present");
        assert!(matches!(
            age.action.text,
            Some(TextRedaction::Clamp { ceiling: Some(c), .. }) if (c - 90.0).abs() < f64::EPSILON,
        ));
        for date_label in ["date_of_birth", "individual_date"] {
            let entry = operators
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
    fn limited_data_set_keeps_dates_ages_and_coarse_geography() {
        // The whole point of LDS is that dates, ages, and coarse
        // geography survive. If any of these ends up in the LDS
        // erase set, the template is broken.
        for survivor in [
            "age",
            "date_of_birth",
            "individual_date",
            "date_time",
            "postal_code",
        ] {
            assert!(
                !LDS_LABELS.iter().any(|l| l.as_str() == survivor),
                "LDS must not erase `{survivor}` — it's a survivor per §164.514(e)(2)",
            );
        }
    }

    #[test]
    fn every_method_ships_a_distinct_policy_identity() {
        let sh = template(HipaaDeidMethod::SafeHarbor);
        let lds = template(HipaaDeidMethod::LimitedDataSet);
        let ed = template(HipaaDeidMethod::ExpertDetermination);
        // Distinct template ids and policy ids across all three
        // — audits key on these to tell the postures apart.
        for (a, b) in [(&sh, &lds), (&sh, &ed), (&lds, &ed)] {
            assert_ne!(a.id, b.id, "template ids must differ");
            assert_ne!(a.policy.id, b.policy.id, "policy ids must differ");
        }
    }

    #[test]
    fn expert_determination_pseudonymizes_the_bulk_group() {
        // The whole point of the ED scaffold vs Safe Harbor is
        // that the bulk terminal is Pseudonymize (identity-
        // preserving), not Erase.
        let RuleDispatch::Predicated { action, .. } =
            &template(HipaaDeidMethod::ExpertDetermination).policy.rules[1].dispatch
        else {
            panic!("second rule must be Predicated dispatch");
        };
        assert!(matches!(action.text, Some(TextRedaction::Pseudonymize)));
    }

    #[test]
    fn template_ids_are_stable_across_calls() {
        for method in [
            HipaaDeidMethod::SafeHarbor,
            HipaaDeidMethod::LimitedDataSet,
            HipaaDeidMethod::ExpertDetermination,
        ] {
            let a = template(method);
            let b = template(method);
            assert_eq!(a.id, b.id);
            assert_eq!(a.policy.id, b.policy.id);
            for (r_a, r_b) in a.policy.rules.iter().zip(b.policy.rules.iter()) {
                assert_eq!(r_a.id, r_b.id);
            }
        }
    }
}
