use elide_governance::redaction::{ModalityRedactions, TextRedaction};
use elide_governance::{LabelScope, PolicyDefinition};
use semver::Version;

use super::super::{derived_id, origin};
use super::{
    EFFECTIVE_DATE, GdprSensitiveScope, Template, article_9_attribution,
    article_9_group_description, template_id,
};

/// Group name Erase's bulk rule references.
const ERASE_SCOPE_NAME: &str = "gdpr_article_9_erase";

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
        scopes: vec![label_scope(scope)],
        custom: Vec::new(),
        // No rules: the whole scope gets one treatment.
        rules: Vec::new(),
        fallback: Some(ModalityRedactions::text(TextRedaction::Erase)),
    }
}

fn label_scope(scope: GdprSensitiveScope) -> LabelScope {
    LabelScope {
        name: ERASE_SCOPE_NAME.into(),
        description: Some(article_9_group_description().into()),
        attribution: Some(article_9_attribution()),
        labels: scope.labels(),
    }
}
