use elide_core::entity::LabelRef;
use elide_governance::Predicate;
use elide_governance::redaction::{ModalityRedactions, TextRedaction};
use elide_governance::{LabelGroup, Labels, PolicyDefinition, PolicyRule, RuleDispatch};
use semver::Version;
use uuid::{Uuid, uuid};

use super::{EFFECTIVE_DATE, HipaaAccountNumbers, Template};

/// Group name Limited Data Set's bulk-erase rule references.
const LDS_GROUP: &str = "hipaa_limited_data_set";

const LDS_POLICY_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000004");
const LDS_BULK_RULE_ID: Uuid = uuid!("0197c348-8800-7000-8000-000000000005");

/// Every label the Limited Data Set bulk-erase rule targets.
/// The sixteen direct-identifier categories §164.514(e)(2)
/// enumerates — dates, ages, town/city, state, and ZIP survive
/// (dropped from this list vs. Safe Harbor's).
///
/// §(e)(2)(ii) excludes "postal address information, other than
/// town or city, State, and zip code", so both `street_address`
/// and the coarser `address` blob erase. Erasing the blob costs
/// the town/city and ZIP inside it, which §(e)(2)(ii) would have
/// let survive — the conservative trade, since letting it through
/// would leak a full street address under a policy claiming
/// §(e)(2) compliance. Enable elide's address-split patterns to
/// recover the survivors.
///
/// `bank_account`, `iban`, `payment_card`, and (with the
/// Extended tier) `crypto_address` are appended per-request from
/// [`HipaaAccountNumbers::labels`] — §164.514(e)(2)(x)
/// treats account numbers the same as Safe Harbor's §(J).
pub(super) const LDS_LABELS: &[LabelRef] = &[
    LabelRef::from_static("person_name"),
    LabelRef::from_static("street_address"),
    LabelRef::from_static("address"),
    LabelRef::from_static("phone_number"),
    LabelRef::from_static("fax_number"),
    LabelRef::from_static("email_address"),
    LabelRef::from_static("government_id"),
    LabelRef::from_static("national_insurance_number"),
    LabelRef::from_static("medical_id"),
    LabelRef::from_static("insurance_id"),
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
    // §164.514(e)(2)(xvi) full face photographic images and any
    // comparable images.
    LabelRef::from_static("face"),
    LabelRef::from_static("internal_id"),
    LabelRef::from_static("case_number"),
    // Not an §164.514(e)(2) category. The LDS list is sixteen
    // enumerated direct identifiers with no residual catch-all —
    // that clause is Safe Harbor's §(b)(2)(i)(R), and its absence
    // here is why an LDS is still PHI requiring a DUA. Retained as
    // a defensive default; drop it for a strict-reading LDS.
    LabelRef::from_static("unresolved"),
];

/// `LDS_LABELS` fused with the caller's account tier.
fn labels(accounts: HipaaAccountNumbers) -> Vec<LabelRef> {
    LDS_LABELS
        .iter()
        .chain(accounts.labels().iter())
        .cloned()
        .collect()
}

pub(super) fn limited_data_set_template(accounts: HipaaAccountNumbers) -> Template {
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
        policy: limited_data_set_policy(accounts),
    }
}

fn limited_data_set_policy(accounts: HipaaAccountNumbers) -> PolicyDefinition {
    PolicyDefinition {
        id: LDS_POLICY_ID,
        name: "hipaa-limited-data-set".into(),
        description: Some(
            "HIPAA Limited Data Set. Sixteen identifier categories erase; dates, \
             ages, town/city, state, and ZIP survive verbatim."
                .into(),
        ),
        labels: Labels {
            builtins: labels(accounts),
            custom: Vec::new(),
        },
        groups: vec![lds_group(accounts)],
        rules: vec![lds_bulk_erase_rule()],
        fallback: None,
    }
}

fn lds_group(accounts: HipaaAccountNumbers) -> LabelGroup {
    LabelGroup {
        name: LDS_GROUP.into(),
        description: Some(
            "The 16 identifier categories §164.514(e)(2) enumerates for the \
             Limited Data Set. Dates, ages, town/city, state, and ZIP survive \
             verbatim under this posture; a Data Use Agreement governs the \
             recipient's use out-of-band."
                .into(),
        ),
        labels: labels(accounts),
    }
}

/// Everything the Limited Data Set group covers → [`Erase`].
///
/// [`Erase`]: elide_governance::redaction::TextRedaction::Erase
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
