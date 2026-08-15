use elide_governance::predicate::Predicate;
use elide_governance::redaction::{ModalityRedactions, TextRedaction};
use elide_governance::{LabelGroup, Labels, PolicyDefinition, PolicyRule, RuleDispatch};
use semver::Version;
use uuid::{Uuid, uuid};

use super::{EFFECTIVE_DATE, GdprArticle9, Template, article_9_group_description};

/// Group name Erase's bulk rule references.
const ERASE_GROUP: &str = "gdpr_article_9_erase";

const ERASE_POLICY_ID: Uuid = uuid!("01639498-5000-7000-8000-000000000001");
const ERASE_RULE_ID: Uuid = uuid!("01639498-5000-7000-8000-000000000002");

pub(super) fn template(cfg: GdprArticle9) -> Template {
    Template {
        id: "gdpr_article_9_erase".into(),
        name: "GDPR Article 9 special categories — erase".into(),
        version: Version::new(1, 0, 0),
        effective_date: EFFECTIVE_DATE,
        description: Some(
            "Erase the nine categories of personal data Article 9(1) treats as special. \
             `scope` widens coverage with re-identification quasi-identifiers \
             and Article 10 criminal-justice labels."
                .into(),
        ),
        policy: policy(cfg),
    }
}

fn policy(cfg: GdprArticle9) -> PolicyDefinition {
    PolicyDefinition {
        id: ERASE_POLICY_ID,
        name: "gdpr-article-9-erase".into(),
        description: Some(
            "Erase every Article 9(1) special-category entity by default. The posture for \
             callers without an Article 9(2) lawful-basis carve-out."
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
        name: ERASE_GROUP.into(),
        description: Some(article_9_group_description().into()),
        labels: cfg.labels(),
    }
}

fn rule() -> PolicyRule {
    PolicyRule {
        id: ERASE_RULE_ID,
        name: "gdpr-article-9-erase".into(),
        description: Some(
            "Erase any entity whose label falls in the Article 9 special-category group.".into(),
        ),
        dispatch: RuleDispatch::Predicated {
            predicate: Predicate::LabelInGroup {
                group: ERASE_GROUP.to_owned(),
            },
            action: Box::new(ModalityRedactions::text(TextRedaction::Erase)),
        },
    }
}
