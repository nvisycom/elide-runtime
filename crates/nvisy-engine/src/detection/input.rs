//! [`DetectionInput`]: arguments to [`DetectionEngine::detect`].
//!
//! [`DetectionEngine::detect`]: crate::detection::DetectionEngine::detect

use std::collections::{HashMap, HashSet};

use hipstr::HipStr;
use nvisy_core::Error;
use nvisy_core::entity::EntityLabelCatalog;
use uuid::Uuid;

use crate::core::ingestion::ImportFile;
use crate::detection::DetectionPlan;
use crate::policy::{Action, Policy};

const TARGET: &str = "nvisy_engine::pipeline::detection::input";

/// Input required to execute a detection pass.
///
/// A detection pass runs imports → extraction → recognition →
/// deduplication → policy evaluation. It stops before applying
/// any redaction — the [`DetectionResult`] holds the audits with
/// their `Execution::Pending` decisions for the caller to review.
///
/// Export sinks live on the matching [`RedactionInput`] instead.
///
/// [`DetectionResult`]: super::DetectionResult
/// [`RedactionInput`]: super::super::redaction::RedactionInput
#[derive(Clone)]
pub struct DetectionInput {
    /// Identity of the human or service account initiating the run.
    pub actor_id: Uuid,
    /// Policies to apply, in precedence order: index `0` is highest
    /// precedence. Submitted inline with their full rule bodies —
    /// the engine does not persist policies as a resource. Callers
    /// reuse policies by re-submitting the same bytes.
    pub policies: Vec<Policy>,
    /// Content sources to ingest at the start of the run.
    pub imports: Vec<ImportFile>,
    /// Per-phase behaviour knobs the detection pipeline reads for
    /// each document.
    pub plan: DetectionPlan,
}

impl DetectionInput {
    /// Validate the name uniqueness invariants the audit's
    /// [`PolicyDecisionRef`] depends on: no two submitted policies
    /// share a name, no two rules inside one policy share a name.
    /// Audit decisions reference rules by policy + rule name, so
    /// duplicates would make those references ambiguous.
    ///
    /// # Errors
    ///
    /// Returns a validation error naming the offending duplicate.
    ///
    /// [`PolicyDecisionRef`]: crate::policy::PolicyDecisionRef
    pub fn validate_namespace(&self) -> Result<(), Error> {
        let mut seen_policies: HashSet<&str> = HashSet::with_capacity(self.policies.len());
        for policy in &self.policies {
            let policy_name = policy.name.as_str();
            if !seen_policies.insert(policy_name) {
                return Err(Error::validation(
                    format!(
                        "duplicate policy name `{policy_name}` in detection submission; \
                         audit references rules by policy + rule name, so policy \
                         names must be unique within a single detect call",
                    ),
                    TARGET,
                ));
            }
            let mut seen_rules: HashSet<&str> = HashSet::with_capacity(policy.rules.len());
            for rule in &policy.rules {
                let rule_name = rule.name.as_str();
                if !seen_rules.insert(rule_name) {
                    return Err(Error::validation(
                        format!(
                            "policy `{policy_name}` has duplicate rule name `{rule_name}`; \
                             rule names must be unique within their owning policy",
                        ),
                        TARGET,
                    ));
                }
            }
        }
        Ok(())
    }

    /// Union every submitted policy's [`Policy::labels`] into a
    /// per-request [`EntityLabelCatalog`]. The returned catalog
    /// drives recognizer dispatch (NER label list, pattern
    /// filtering) and selector tag matching for this request.
    ///
    /// Two policies declaring the same label name with non-equal
    /// `(description, tags)` are a conflict — the engine cannot
    /// pick a winner without violating one author's intent.
    /// Identical re-declarations of the same label are allowed
    /// (idempotent).
    ///
    /// # Errors
    ///
    /// Returns a validation error naming both policies on the
    /// first conflict encountered.
    pub fn unify_labels(&self) -> Result<EntityLabelCatalog, Error> {
        let mut catalog = EntityLabelCatalog::new();
        let mut origin: HashMap<HipStr<'static>, HipStr<'static>> = HashMap::new();

        for policy in &self.policies {
            for label in &policy.labels {
                if let Some(existing) = catalog.lookup(label.name.as_str()) {
                    if existing != label {
                        let prior = origin
                            .get(&label.name)
                            .map(HipStr::as_str)
                            .unwrap_or("<unknown>");
                        return Err(Error::validation(
                            format!(
                                "policy `{}` redeclares label `{}` with a different body \
                                 than policy `{prior}`; resolve the divergence so both \
                                 policies share one definition",
                                policy.name, label.name,
                            ),
                            TARGET,
                        ));
                    }
                    continue;
                }
                origin.insert(label.name.clone(), policy.name.clone());
                catalog.insert(label.clone());
            }
        }
        Ok(catalog)
    }

    /// Validate that every [`EntitySelector::labels`] entry across
    /// every rule references a label registered in `catalog`.
    /// Selectors targeting unregistered names would never match —
    /// silently dropping them would let a typo bypass redaction.
    ///
    /// # Errors
    ///
    /// Returns a validation error naming the policy, rule, and
    /// missing label on the first violation encountered.
    ///
    /// [`EntitySelector::labels`]: crate::policy::EntitySelector::labels
    pub fn validate_selector_labels(&self, catalog: &EntityLabelCatalog) -> Result<(), Error> {
        for policy in &self.policies {
            for rule in &policy.rules {
                for label in &rule.selector.labels {
                    if catalog.lookup(label.as_str()).is_none() {
                        return Err(Error::validation(
                            format!(
                                "policy `{}` rule `{}` selects label `{}` \
                                 which no policy declares; declare it on `policy.labels` \
                                 or remove the selector entry",
                                policy.name,
                                rule.name,
                                label.as_str(),
                            ),
                            TARGET,
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Validate every `Action::Redact` action carries at least one
    /// per-modality operator. A rule that says "redact" but
    /// specifies no operators is an author bug — the apply phase
    /// would either fall through to the default operator (which
    /// might not exist either) or silently skip the entity.
    /// Either way, the author meant *something* and didn't say
    /// what — reject loudly.
    ///
    /// # Errors
    ///
    /// Returns a validation error naming the policy and rule on
    /// the first empty `Redact` action encountered.
    pub fn validate_actions(&self) -> Result<(), Error> {
        for policy in &self.policies {
            for rule in &policy.rules {
                check_action(&rule.action, &policy.name, Some(&rule.name))?;
            }
            if let Some(action) = policy.default_action.as_ref() {
                check_action(action, &policy.name, None)?;
            }
        }
        Ok(())
    }
}

fn check_action(action: &Action, policy_name: &str, rule_name: Option<&str>) -> Result<(), Error> {
    if let Action::Redact(operators) = action
        && operators.is_empty()
    {
        let location = match rule_name {
            Some(r) => format!("rule `{r}`"),
            None => "default_action".to_string(),
        };
        return Err(Error::validation(
            format!(
                "policy `{policy_name}` {location} declares `redact` but specifies no \
                 per-modality operators; add at least one of `text`, `tabular`, \
                 `image`, `audio`",
            ),
            TARGET,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use nvisy_core::entity::{EntityLabel, EntityLabelRef};
    use semver::Version;

    use super::*;
    use crate::policy::redaction::{ModalityRedactions, TextRedaction};
    use crate::policy::{EntitySelector, PolicyRule, SuppressAction};

    fn label(name: &str, tags: &[&'static str]) -> EntityLabel {
        EntityLabel::new(name.to_owned()).with_tags(tags.iter().copied())
    }

    fn policy(name: &str, rules: &[&str], labels: Vec<EntityLabel>) -> Policy {
        let rules = rules
            .iter()
            .map(|n| PolicyRule {
                name: HipStr::from(*n),
                selector: EntitySelector::default(),
                action: Action::Suppress(SuppressAction::default()),
                conditions: Vec::new(),
                enabled: true,
            })
            .collect();
        Policy {
            name: HipStr::from(name),
            version: Version::new(1, 0, 0),
            description: None,
            labels,
            rules,
            default_action: None,
            retention: Vec::new(),
        }
    }

    fn policy_with_selector(name: &str, selector_labels: &[&str]) -> Policy {
        let labels: Vec<EntityLabelRef> = selector_labels
            .iter()
            .map(|s| EntityLabelRef::new(HipStr::from(s.to_owned())))
            .collect();
        Policy {
            name: HipStr::from(name),
            version: Version::new(1, 0, 0),
            description: None,
            labels: Vec::new(),
            rules: vec![PolicyRule {
                name: HipStr::from("r"),
                selector: EntitySelector {
                    labels,
                    ..EntitySelector::default()
                },
                action: Action::Suppress(SuppressAction::default()),
                conditions: Vec::new(),
                enabled: true,
            }],
            default_action: None,
            retention: Vec::new(),
        }
    }

    fn input(policies: Vec<Policy>) -> DetectionInput {
        DetectionInput {
            actor_id: Uuid::nil(),
            policies,
            imports: Vec::new(),
            plan: DetectionPlan::default(),
        }
    }

    #[test]
    fn validate_namespace_empty_passes() {
        assert!(input(Vec::new()).validate_namespace().is_ok());
    }

    #[test]
    fn validate_namespace_distinct_policies_pass() {
        let r = input(vec![
            policy("gdpr", &["redact-ssn", "redact-email"], Vec::new()),
            policy("hipaa", &["redact-mrn"], Vec::new()),
        ])
        .validate_namespace();
        assert!(r.is_ok());
    }

    #[test]
    fn validate_namespace_duplicate_policy_name_fails() {
        let err = input(vec![
            policy("gdpr", &["a"], Vec::new()),
            policy("gdpr", &["b"], Vec::new()),
        ])
        .validate_namespace()
        .unwrap_err();
        assert!(err.to_string().contains("duplicate policy name `gdpr`"));
    }

    #[test]
    fn validate_namespace_duplicate_rule_name_fails() {
        let err = input(vec![policy("gdpr", &["dup", "dup"], Vec::new())])
            .validate_namespace()
            .unwrap_err();
        assert!(err.to_string().contains("duplicate rule name `dup`"));
    }

    #[test]
    fn unify_labels_empty_input_is_empty_catalog() {
        let cat = input(Vec::new()).unify_labels().unwrap();
        assert!(cat.is_empty());
    }

    #[test]
    fn unify_labels_unions_disjoint_policies() {
        let cat = input(vec![
            policy("gdpr", &[], vec![label("email_address", &["pii"])]),
            policy("hipaa", &[], vec![label("diagnosis", &["phi"])]),
        ])
        .unify_labels()
        .unwrap();
        assert_eq!(cat.len(), 2);
    }

    #[test]
    fn unify_labels_idempotent_redeclaration_passes() {
        let lbl = label("email_address", &["pii"]);
        let cat = input(vec![
            policy("gdpr", &[], vec![lbl.clone()]),
            policy("ccpa", &[], vec![lbl]),
        ])
        .unify_labels()
        .unwrap();
        assert_eq!(cat.len(), 1);
    }

    #[test]
    fn unify_labels_conflicting_redeclaration_fails() {
        let err = input(vec![
            policy("gdpr", &[], vec![label("email_address", &["pii"])]),
            policy(
                "ccpa",
                &[],
                vec![label("email_address", &["pii", "contact_info"])],
            ),
        ])
        .unify_labels()
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ccpa") && msg.contains("gdpr") && msg.contains("email_address"));
    }

    #[test]
    fn validate_selector_labels_passes_when_label_in_catalog() {
        let catalog = EntityLabelCatalog::new().with_label(label("email_address", &["pii"]));
        let i = input(vec![policy_with_selector("gdpr", &["email_address"])]);
        assert!(i.validate_selector_labels(&catalog).is_ok());
    }

    #[test]
    fn validate_selector_labels_fails_on_missing_label() {
        let catalog = EntityLabelCatalog::new();
        let i = input(vec![policy_with_selector("gdpr", &["email_address"])]);
        let err = i.validate_selector_labels(&catalog).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("email_address") && msg.contains("gdpr"));
    }

    #[test]
    fn validate_actions_passes_with_non_empty_redact() {
        let mut p = policy("gdpr", &[], Vec::new());
        p.rules.push(PolicyRule {
            name: HipStr::from("r"),
            selector: EntitySelector::default(),
            action: Action::Redact(ModalityRedactions {
                text: Some(TextRedaction::Redact),
                ..Default::default()
            }),
            conditions: Vec::new(),
            enabled: true,
        });
        assert!(input(vec![p]).validate_actions().is_ok());
    }

    #[test]
    fn validate_actions_fails_on_empty_redact() {
        let mut p = policy("gdpr", &[], Vec::new());
        p.rules.push(PolicyRule {
            name: HipStr::from("empty"),
            selector: EntitySelector::default(),
            action: Action::Redact(ModalityRedactions::default()),
            conditions: Vec::new(),
            enabled: true,
        });
        let err = input(vec![p]).validate_actions().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("empty") && msg.contains("gdpr"));
    }
}
