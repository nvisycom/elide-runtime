use elide_core::entity::LabelRef;
use nvisy_policy::predicate::Predicate;
use nvisy_policy::redaction::{
    ClampBucket, DateGranularity, DateStyle, ModalityRedactions, TextRedaction,
};
use nvisy_policy::{LabelEntry, LabelGroup, Labels, PolicyDefinition, PolicyRule, RuleDispatch};
use semver::Version;
use uuid::{Uuid, uuid};

use super::{EFFECTIVE_DATE, HipaaAccountNumbers, Template};

/// Group name Safe Harbor's bulk-erase rule references.
pub(super) const SAFE_HARBOR_GROUP: &str = "hipaa_safe_harbor";

pub(super) const SAFE_HARBOR_POLICY_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000001");
pub(super) const SAFE_HARBOR_TABLE_RULE_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000002");
pub(super) const SAFE_HARBOR_BULK_RULE_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000003");

/// Every label the Safe Harbor bulk-erase rule targets.
/// `age`, `date_of_birth`, `individual_date` are absent — the
/// table rule owns them.
pub(super) const SAFE_HARBOR_LABELS: &[LabelRef] = &[
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
    // §(J) account numbers are appended per-request from
    // `HipaaAccountNumbers::labels`, so the caller's
    // Standard/Extended pick decides whether crypto addresses
    // count. `bank_account`, `iban`, and `payment_card` land
    // via Standard; `crypto_address` via Extended.
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
pub(super) const SAFE_HARBOR_TABLE_LABELS: &[LabelRef] = &[
    LabelRef::from_static("age"),
    LabelRef::from_static("date_of_birth"),
    LabelRef::from_static("individual_date"),
];

/// `SAFE_HARBOR_LABELS` fused with the caller's §(J) account
/// tier — the full Safe Harbor label set. Shared with the
/// Expert Determination posture, which carries the same
/// 18-identifier scope.
pub(super) fn labels(accounts: HipaaAccountNumbers) -> Vec<LabelRef> {
    SAFE_HARBOR_LABELS
        .iter()
        .chain(accounts.labels().iter())
        .cloned()
        .collect()
}

pub(super) fn safe_harbor_template(accounts: HipaaAccountNumbers) -> Template {
    Template {
        id: "hipaa_deid_safe_harbor".into(),
        name: "HIPAA §164.514(b)(2) Safe Harbor".into(),
        version: Version::new(1, 0, 0),
        effective_date: EFFECTIVE_DATE,
        description: Some(
            "Remove the eighteen identifier categories the Safe Harbor rule enumerates.".into(),
        ),
        policy: safe_harbor_policy(accounts),
    }
}

fn safe_harbor_policy(accounts: HipaaAccountNumbers) -> PolicyDefinition {
    PolicyDefinition {
        id: SAFE_HARBOR_POLICY_ID,
        name: "hipaa-safe-harbor".into(),
        description: Some(
            "HIPAA Safe Harbor de-identification. Ages ≥ 90 collapse to a bucket, \
             dates reduce to the year, every other identifier is erased."
                .into(),
        ),
        labels: Labels {
            builtins: labels(accounts)
                .into_iter()
                .chain(SAFE_HARBOR_TABLE_LABELS.iter().cloned())
                .collect(),
            custom: Vec::new(),
        },
        groups: vec![safe_harbor_group(accounts)],
        rules: vec![safe_harbor_table_rule(), safe_harbor_bulk_erase_rule()],
        fallback: None,
    }
}

fn safe_harbor_group(accounts: HipaaAccountNumbers) -> LabelGroup {
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
        labels: labels(accounts),
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
