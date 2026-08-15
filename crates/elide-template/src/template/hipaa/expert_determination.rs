use elide_core::entity::LabelRef;
use elide_governance::redaction::{ClampBucket, ModalityRedactions, TextRedaction};
use elide_governance::{
    LabelEntry, LabelGroup, Labels, PolicyDefinition, PolicyRule, Predicate, RuleDispatch,
};
use elide_operator::operators::{DateGranularity, DateStyle};
use semver::Version;

use super::super::{cited, derived_id, origin};
use super::safe_harbor::{SAFE_HARBOR_TABLE_LABELS, labels as safe_harbor_labels};
use super::{EFFECTIVE_DATE, HipaaAccountNumbers, Template, template_id};

/// Group name Expert Determination's bulk-pseudonymize rule
/// references. Same label membership as Safe Harbor; the
/// separate group name lets audits distinguish the two postures
/// by the group id alone.
const ED_GROUP: &str = "hipaa_expert_determination";

/// Machine key for the Expert Determination scaffold, before the
/// account tier is folded in.
const ED_ID: &str = "hipaa_deid_expert_determination";

pub(super) fn expert_determination_template(accounts: HipaaAccountNumbers) -> Template {
    Template {
        id: template_id(ED_ID, accounts).into(),
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
        policy: expert_determination_policy(accounts),
    }
}

fn expert_determination_policy(accounts: HipaaAccountNumbers) -> PolicyDefinition {
    PolicyDefinition {
        id: derived_id(&format!("{}:policy", template_id(ED_ID, accounts))),
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
        template: Some(origin(
            "hipaa_deid_expert_determination",
            Version::new(1, 0, 0),
        )),
        labels: Labels {
            builtins: safe_harbor_labels(accounts)
                .into_iter()
                .chain(SAFE_HARBOR_TABLE_LABELS.iter().cloned())
                .collect(),
            custom: Vec::new(),
        },
        groups: vec![ed_group(accounts)],
        rules: vec![ed_table_rule(accounts), ed_bulk_pseudonymize_rule(accounts)],
        fallback: None,
    }
}

fn ed_group(accounts: HipaaAccountNumbers) -> LabelGroup {
    LabelGroup {
        name: ED_GROUP.into(),
        description: Some(
            "Safe Harbor's 18 identifier categories, carried into the Expert \
             Determination scaffold as a starting scope: §164.514(b)(1) itself \
             enumerates no identifiers, leaving the actual scope to the \
             statistician's risk analysis. Same label membership as Safe Harbor; \
             the separate group name lets audits distinguish the two postures by \
             group id alone."
                .into(),
        ),
        attribution: Some(cited(
            "HIPAA",
            "§164.514(b)(1)",
            "a starting scope borrowed from Safe Harbor; the provision itself \
             enumerates no identifiers, leaving scope to the statistician",
        )),
        labels: safe_harbor_labels(accounts),
    }
}

/// Same table dispatch as Safe Harbor: ages ≥ 90 collapse to
/// the bucket, dates reduce to the year: carried into the
/// Expert Determination scaffold as sensible defaults the
/// statistician can override.
fn ed_table_rule(accounts: HipaaAccountNumbers) -> PolicyRule {
    PolicyRule {
        id: derived_id(&format!(
            "{}:rule:age-and-dates",
            template_id(ED_ID, accounts)
        )),
        name: "hipaa-ed-age-and-dates".into(),
        description: Some(
            "Age/date dispatch carried into the Expert Determination scaffold. \
             The statistician's risk analysis may override these defaults."
                .into(),
        ),
        attribution: Some(cited(
            "HIPAA",
            "§164.514(b)(1)",
            "age and date defaults carried from Safe Harbor pending the \
             statistician's risk analysis, which may widen or narrow them",
        )),
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
/// [`Pseudonymize`]: elide_governance::redaction::TextRedaction::Pseudonymize
fn ed_bulk_pseudonymize_rule(accounts: HipaaAccountNumbers) -> PolicyRule {
    PolicyRule {
        id: derived_id(&format!(
            "{}:rule:bulk-pseudonymize",
            template_id(ED_ID, accounts)
        )),
        name: "hipaa-ed-bulk-pseudonymize".into(),
        description: Some(
            "Every identifier in the Expert Determination label set is \
             pseudonymized (identity-preserving surrogate). Statistician may \
             override to Erase / HmacHash / Encrypt as their risk analysis \
             demands."
                .into(),
        ),
        attribution: Some(cited(
            "HIPAA",
            "§164.514(b)(1)",
            "identity preserved across mentions for the analytics the expert \
             determination path exists to serve; does not itself certify \
             de-identification, which requires the statistician's attestation",
        )),
        dispatch: RuleDispatch::Predicated {
            predicate: Predicate::LabelInGroup {
                group: ED_GROUP.to_owned(),
            },
            action: Box::new(ModalityRedactions::text(TextRedaction::Pseudonymize)),
        },
    }
}
