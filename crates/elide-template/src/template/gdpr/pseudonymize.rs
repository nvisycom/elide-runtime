use elide_governance::policy::{LabelScope, Policy};
use elide_governance::redaction::{ModalityRedactions, TextRedaction};
use semver::Version;

use super::super::{derived_id, origin};
use super::{
    EFFECTIVE_DATE, GdprSensitiveScope, Template, article_9_attribution,
    article_9_group_description, template_id,
};

/// Group name Pseudonymize's bulk rule references. Separate name
/// from Erase's scope so audits distinguish the two postures by
/// scope name alone.
const PSEUDONYMIZE_SCOPE_NAME: &str = "gdpr_article_9_pseudonymize";

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

fn policy(scope: GdprSensitiveScope) -> Policy {
    Policy {
        id: derived_id(&format!("{}:policy", template_id(PSEUDONYMIZE_ID, scope))),
        name: "gdpr-article-9-pseudonymize".into(),
        description: Some(
            "Pseudonymize every Article 9(1) special-category entity (identity-preserving \
             surrogate). Requires an Article 9(2) lawful-basis carve-out established \
             out-of-band; the template does not verify or record the basis."
                .into(),
        ),
        template: Some(origin("gdpr_article_9_pseudonymize", Version::new(1, 0, 0))),
        scopes: vec![label_scope(scope)],
        // No rules: the whole scope gets one treatment.
        fallback: Some(ModalityRedactions::textual(TextRedaction::Pseudonymize)),
        ..Policy::default()
    }
}

fn label_scope(scope: GdprSensitiveScope) -> LabelScope {
    LabelScope {
        name: PSEUDONYMIZE_SCOPE_NAME.into(),
        description: Some(article_9_group_description().into()),
        attribution: Some(article_9_attribution()),
        labels: scope.labels(),
    }
}
