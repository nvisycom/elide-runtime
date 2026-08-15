use elide_governance::predicate::Predicate;
use elide_governance::redaction::{ModalityRedactions, TextRedaction};
use elide_governance::{LabelGroup, Labels, PolicyDefinition, PolicyRule, RuleDispatch};
use semver::Version;
use uuid::{Uuid, uuid};

use super::{EFFECTIVE_DATE, GdprArticle9, Template, article_9_group_description};

/// Group name Pseudonymize's bulk rule references. Separate name
/// from Erase's group so audits distinguish the two postures by
/// group id alone.
const PSEUDONYMIZE_GROUP: &str = "gdpr_article_9_pseudonymize";

const PSEUDONYMIZE_POLICY_ID: Uuid = uuid!("01639498-5000-7000-8000-000000000003");
const PSEUDONYMIZE_RULE_ID: Uuid = uuid!("01639498-5000-7000-8000-000000000004");

pub(super) fn template(cfg: GdprArticle9) -> Template {
    Template {
        id: "gdpr_article_9_pseudonymize".into(),
        name: "GDPR Article 9 special categories — pseudonymize".into(),
        version: Version::new(1, 0, 0),
        effective_date: EFFECTIVE_DATE,
        description: Some(
            "Pseudonymize the nine categories of personal data Article 9(1) treats as special. \
             Requires an Article 9(2) lawful-basis carve-out established out-of-band. \
             `scope` widens coverage with Recital 26 re-identification quasi-identifiers \
             and Article 10 criminal-justice labels."
                .into(),
        ),
        policy: policy(cfg),
    }
}

fn policy(cfg: GdprArticle9) -> PolicyDefinition {
    PolicyDefinition {
        id: PSEUDONYMIZE_POLICY_ID,
        name: "gdpr-article-9-pseudonymize".into(),
        description: Some(
            "Pseudonymize every Article 9(1) special-category entity (identity-preserving \
             surrogate). Requires an Article 9(2) lawful-basis carve-out established \
             out-of-band; the template does not verify or record the basis."
                .into(),
        ),
        labels: Labels {
            builtins: cfg.labels(),
            custom: Vec::new(),
        },
        groups: vec![group(cfg)],
        rules: vec![rule()],
        fallback: None,
    }
}

fn group(cfg: GdprArticle9) -> LabelGroup {
    LabelGroup {
        name: PSEUDONYMIZE_GROUP.into(),
        description: Some(article_9_group_description().into()),
        labels: cfg.labels(),
    }
}

fn rule() -> PolicyRule {
    PolicyRule {
        id: PSEUDONYMIZE_RULE_ID,
        name: "gdpr-article-9-pseudonymize".into(),
        description: Some(
            "Pseudonymize any entity whose label falls in the Article 9 special-category \
             group (identity-preserving surrogate)."
                .into(),
        ),
        dispatch: RuleDispatch::Predicated {
            predicate: Predicate::LabelInGroup {
                group: PSEUDONYMIZE_GROUP.to_owned(),
            },
            action: Box::new(ModalityRedactions::text(TextRedaction::Pseudonymize)),
        },
    }
}
