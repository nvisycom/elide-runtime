use elide_governance::redaction::{ModalityRedactions, TextRedaction};
use elide_governance::{LabelGroup, Labels, PolicyDefinition, PolicyRule, Predicate, RuleDispatch};
use semver::Version;

use super::super::{cited, derived_id, origin};
use super::{
    EFFECTIVE_DATE, GdprSensitiveScope, Template, article_9_attribution,
    article_9_group_description, template_id,
};

/// Group name Erase's bulk rule references.
const ERASE_GROUP: &str = "gdpr_article_9_erase";

/// Machine key for this posture, before the scope is folded in.
const ERASE_ID: &str = "gdpr_article_9_erase";

pub(super) fn template(scope: GdprSensitiveScope) -> Template {
    Template {
        id: template_id(ERASE_ID, scope).into(),
        name: "GDPR Article 9 special categories: erase".into(),
        version: Version::new(1, 0, 0),
        effective_date: EFFECTIVE_DATE,
        description: Some(
            "Erase the nine categories of personal data Article 9(1) treats as special. \
             `scope` widens coverage with re-identification quasi-identifiers \
             and Article 10 criminal-justice labels."
                .into(),
        ),
        policy: policy(scope),
    }
}

fn policy(scope: GdprSensitiveScope) -> PolicyDefinition {
    PolicyDefinition {
        id: derived_id(&format!("{}:policy", template_id(ERASE_ID, scope))),
        name: "gdpr-article-9-erase".into(),
        description: Some(
            "Erase every Article 9(1) special-category entity by default. The posture for \
             callers without an Article 9(2) lawful-basis carve-out."
                .into(),
        ),
        template: Some(origin("gdpr_article_9_erase", Version::new(1, 0, 0))),
        labels: Labels {
            builtins: scope.labels(),
            custom: Vec::new(),
        },
        groups: vec![group(scope)],
        rules: vec![rule(scope)],
        fallback: None,
    }
}

fn group(scope: GdprSensitiveScope) -> LabelGroup {
    LabelGroup {
        name: ERASE_GROUP.into(),
        description: Some(article_9_group_description().into()),
        attribution: Some(article_9_attribution()),
        labels: scope.labels(),
    }
}

fn rule(scope: GdprSensitiveScope) -> PolicyRule {
    PolicyRule {
        id: derived_id(&format!("{}:rule:erase", template_id(ERASE_ID, scope))),
        name: "gdpr-article-9-erase".into(),
        description: Some(
            "Erase any entity whose label falls in the Article 9 special-category group.".into(),
        ),
        attribution: Some(cited(
            "GDPR",
            "Article 9(1)",
            "no Article 9(2) carve-out asserted, so special-category data may not \
             be processed and is removed",
        )),
        dispatch: RuleDispatch::Predicated {
            predicate: Predicate::LabelInGroup {
                group: ERASE_GROUP.to_owned(),
            },
            action: Box::new(ModalityRedactions::text(TextRedaction::Erase)),
        },
    }
}
