use elide_governance::redaction::{ModalityRedactions, TextRedaction};
use elide_governance::{LabelGroup, Labels, PolicyDefinition, PolicyRule, Predicate, RuleDispatch};
use semver::Version;

use super::super::{cited, derived_id, origin};
use super::{
    EFFECTIVE_DATE, GdprSensitiveScope, Template, article_9_attribution,
    article_9_group_description, template_id,
};

/// Group name Pseudonymize's bulk rule references. Separate name
/// from Erase's group so audits distinguish the two postures by
/// group id alone.
const PSEUDONYMIZE_GROUP: &str = "gdpr_article_9_pseudonymize";

/// Machine key for this posture, before the scope is folded in.
const PSEUDONYMIZE_ID: &str = "gdpr_article_9_pseudonymize";

pub(super) fn template(scope: GdprSensitiveScope) -> Template {
    Template {
        id: template_id(PSEUDONYMIZE_ID, scope).into(),
        name: "GDPR Article 9 special categories: pseudonymize".into(),
        version: Version::new(1, 0, 0),
        effective_date: EFFECTIVE_DATE,
        description: Some(
            "Pseudonymize the nine categories of personal data Article 9(1) treats as special. \
             Requires an Article 9(2) lawful-basis carve-out established out-of-band. \
             `scope` widens coverage with re-identification quasi-identifiers \
             and Article 10 criminal-justice labels."
                .into(),
        ),
        policy: policy(scope),
    }
}

fn policy(scope: GdprSensitiveScope) -> PolicyDefinition {
    PolicyDefinition {
        id: derived_id(&format!("{}:policy", template_id(PSEUDONYMIZE_ID, scope))),
        name: "gdpr-article-9-pseudonymize".into(),
        description: Some(
            "Pseudonymize every Article 9(1) special-category entity (identity-preserving \
             surrogate). Requires an Article 9(2) lawful-basis carve-out established \
             out-of-band; the template does not verify or record the basis."
                .into(),
        ),
        template: Some(origin("gdpr_article_9_pseudonymize", Version::new(1, 0, 0))),
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
        name: PSEUDONYMIZE_GROUP.into(),
        description: Some(article_9_group_description().into()),
        attribution: Some(article_9_attribution()),
        labels: scope.labels(),
    }
}

fn rule(scope: GdprSensitiveScope) -> PolicyRule {
    PolicyRule {
        id: derived_id(&format!(
            "{}:rule:pseudonymize",
            template_id(PSEUDONYMIZE_ID, scope)
        )),
        name: "gdpr-article-9-pseudonymize".into(),
        description: Some(
            "Pseudonymize any entity whose label falls in the Article 9 special-category \
             group (identity-preserving surrogate)."
                .into(),
        ),
        attribution: Some(cited(
            "GDPR",
            "Article 9(2)",
            "a lawful-basis carve-out permits retention, so identity is preserved \
             across mentions rather than erased",
        )),
        dispatch: RuleDispatch::Predicated {
            predicate: Predicate::LabelInGroup {
                group: PSEUDONYMIZE_GROUP.to_owned(),
            },
            action: Box::new(ModalityRedactions::text(TextRedaction::Pseudonymize)),
        },
    }
}
