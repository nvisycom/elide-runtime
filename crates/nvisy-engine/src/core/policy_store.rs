//! [`PolicyStore`]: per-request flat container of [`Arc<Policy>`],
//! plus the per-entity decision resolver that walks it.
//!
//! Built from a `Vec<Policy>` submission via
//! [`PolicyStore::from_policies`], which consumes the submitted
//! policies and wraps each in an `Arc<Policy>` — no deep clones.
//! Detection and redaction pipelines share a single store via
//! `Arc<PolicyStore>`; per-call handoff is a refcount bump.
//!
//! Policies are flat (no per-modality bucketing); each rule's
//! `Action::Redact(operators)` is itself modality-aware via
//! [`ModalityRedactions::operator_for`]. `resolve::<M>` walks every
//! policy and uses the per-modality projection to pick the operator
//! for the entity's modality, falling back to the deployment-wide
//! defaults when the rule didn't cover that modality.
//!
//! [`SharedData`]: super::SharedData
//! [`ModalityRedactions::operator_for`]: crate::policy::redaction::ModalityRedactions::operator_for

use std::sync::Arc;

use hipstr::HipStr;
use nvisy_codec::content::ContentDescriptor;
use nvisy_core::entity::Entity;

use crate::modality::DocumentModality;
use crate::policy::redaction::{ModalityRedactions, ProjectRedaction};
use crate::policy::{Action, Condition, Policy, PolicyRule};

/// Flat container of [`Arc<Policy>`] in precedence order — index
/// `0` is highest precedence. Shared between detection and redaction
/// pipelines via `Arc<PolicyStore>` so per-call handoff is a
/// refcount bump.
#[derive(Default)]
pub struct PolicyStore {
    policies: Vec<Arc<Policy>>,
}

impl PolicyStore {
    /// Construct a store from a `Vec<Policy>` submission, taking
    /// ownership so each policy moves straight into its [`Arc`] —
    /// no deep clones.
    pub(crate) fn from_policies(policies: Vec<Policy>) -> Self {
        Self {
            policies: policies.into_iter().map(Arc::new).collect(),
        }
    }

    /// Union every stored policy's [`Policy::labels`] into a single
    /// [`EntityLabelCatalog`]. Used at redaction time to rebuild
    /// the per-request catalog the detection pass already validated.
    /// Conflicts here are impossible because the same union was
    /// validated at detection-time submission.
    ///
    /// [`EntityLabelCatalog`]: nvisy_core::entity::EntityLabelCatalog
    pub(crate) fn catalog(&self) -> nvisy_core::entity::EntityLabelCatalog {
        let mut catalog = nvisy_core::entity::EntityLabelCatalog::new();
        for p in &self.policies {
            for l in &p.labels {
                catalog.insert(l.clone());
            }
        }
        catalog
    }

    /// Borrow the full policy chain. Crate-internal because the
    /// public surface is `resolve::<M>`.
    #[cfg(test)]
    fn chain(&self) -> &[Arc<Policy>] {
        &self.policies
    }

    /// Resolve a single entity against the policy chain. Walks
    /// policies in precedence order; within a policy, walks rules
    /// in declaration order. First matching rule wins; when no
    /// rule in a policy matches, falls back to that policy's
    /// [`Policy::default_action`] (if any) before descending to
    /// the next policy.
    ///
    /// `default_operators` is the deployment-wide fallback: when a
    /// rule's `Redact` action doesn't cover the entity's modality
    /// (per [`ModalityRedactions::operator_for`]), the resolver
    /// tries the default before skipping the rule.
    ///
    /// Returns [`Decision::Fallthrough`] when no policy in the
    /// chain produced a decision; the caller's default-threshold
    /// path takes over.
    pub(crate) fn resolve<M: DocumentModality + ProjectRedaction>(
        &self,
        entity: &Entity<M>,
        catalog: &nvisy_core::entity::EntityLabelCatalog,
        default_operators: &ModalityRedactions,
        document_labels: &[&str],
        descriptor: &ContentDescriptor,
    ) -> Decision<M> {
        for policy in &self.policies {
            for rule in &policy.rules {
                if !rule_matches(rule, entity, catalog, document_labels, descriptor) {
                    continue;
                }
                if let Some(decision) = decide::<M>(
                    &rule.action,
                    &policy.name,
                    Some(rule.name.clone()),
                    default_operators,
                ) {
                    return decision;
                }
            }
            if let Some(default) = policy.default_action.as_ref()
                && let Some(decision) = decide::<M>(default, &policy.name, None, default_operators)
            {
                return decision;
            }
        }
        Decision::Fallthrough
    }
}

impl std::fmt::Debug for PolicyStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolicyStore")
            .field("count", &self.policies.len())
            .finish()
    }
}

/// Outcome of walking the policy chain for one entity.
///
/// The winning rule's typed operator spec rides along on
/// [`Decision::Redact`] so the apply phase can instantiate the
/// operator (built-in arms) or look it up in the toolkit
/// [`RedactionRegistry<M>`] (`Custom` arm) without re-walking
/// the policy chain.
///
/// [`RedactionRegistry<M>`]: nvisy_toolkit::redaction::RedactionRegistry
pub(crate) enum Decision<M: DocumentModality> {
    /// A rule chose to redact. `operator` is the per-modality
    /// operator spec the winning rule (or fallback) projected for
    /// this entity's modality. `rule_name` is `None` when the
    /// policy's `default_action` fallback fired.
    Redact {
        policy_name: HipStr<'static>,
        rule_name: Option<HipStr<'static>>,
        operator: M::Redaction,
    },
    /// A `Suppress` rule fired. `reason` is the author-supplied
    /// suppression reason, if any.
    Suppress {
        policy_name: HipStr<'static>,
        rule_name: Option<HipStr<'static>>,
        reason: Option<HipStr<'static>>,
    },
    /// An `Audit` rule fired. The entity is left untouched; the
    /// audit entry carries `severity` for downstream review
    /// tooling.
    Audit {
        policy_name: HipStr<'static>,
        rule_name: Option<HipStr<'static>>,
        severity: Option<HipStr<'static>>,
    },
    /// No policy in the chain produced a decision. The caller
    /// falls back to its default-threshold path.
    Fallthrough,
}

fn decide<M: DocumentModality + ProjectRedaction>(
    action: &Action,
    policy_name: &HipStr<'static>,
    rule_name: Option<HipStr<'static>>,
    default_operators: &ModalityRedactions,
) -> Option<Decision<M>> {
    match action {
        Action::Redact(operators) => {
            let operator = operators
                .operator_for::<M>()
                .or_else(|| default_operators.operator_for::<M>())?;
            Some(Decision::Redact {
                policy_name: policy_name.clone(),
                rule_name,
                operator: operator.clone(),
            })
        }
        Action::Suppress(opts) => Some(Decision::Suppress {
            policy_name: policy_name.clone(),
            rule_name,
            reason: opts.reason.clone(),
        }),
        Action::Audit(opts) => Some(Decision::Audit {
            policy_name: policy_name.clone(),
            rule_name,
            severity: opts.severity.clone(),
        }),
    }
}

fn rule_matches<M: DocumentModality>(
    rule: &PolicyRule,
    entity: &Entity<M>,
    catalog: &nvisy_core::entity::EntityLabelCatalog,
    document_labels: &[&str],
    descriptor: &ContentDescriptor,
) -> bool {
    rule.enabled
        && rule.selector.matches(entity, catalog)
        && rule
            .conditions
            .iter()
            .all(|c| condition_matches(c, document_labels, descriptor))
}

fn condition_matches(
    condition: &Condition,
    document_labels: &[&str],
    descriptor: &ContentDescriptor,
) -> bool {
    match condition {
        Condition::Labels { labels } => labels.iter().all(|label| {
            document_labels
                .iter()
                .any(|doc| doc.eq_ignore_ascii_case(label))
        }),
        Condition::Metadata { key, value } => match descriptor.get_policy_metadata(key) {
            Some(actual) => match value {
                Some(expected) => actual.as_str().is_some_and(|s| s == expected),
                None => true,
            },
            None => false,
        },
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::entity::Entity;
    use nvisy_core::modality::Text;
    use semver::Version;

    use super::*;

    fn empty_policy(name: &str) -> Policy {
        Policy {
            name: HipStr::from(name),
            version: Version::new(1, 0, 0),
            description: None,
            labels: Vec::new(),
            rules: Vec::new(),
            default_action: None,
            retention: Vec::new(),
        }
    }

    #[test]
    fn empty_chain_returns_fallthrough() {
        let store = PolicyStore::default();
        let entity = Entity::<Text>::test_builder(0, 4).test_build();
        let descriptor = ContentDescriptor::new();
        let catalog = nvisy_core::entity::EntityLabelCatalog::new();
        let defaults = ModalityRedactions::default();
        assert!(matches!(
            store.resolve::<Text>(&entity, &catalog, &defaults, &[], &descriptor),
            Decision::Fallthrough
        ));
    }

    #[test]
    fn from_policies_preserves_order() {
        let store = PolicyStore::from_policies(vec![
            empty_policy("a"),
            empty_policy("b"),
            empty_policy("c"),
        ]);
        assert_eq!(store.chain().len(), 3);
        assert_eq!(store.chain()[0].name, "a");
        assert_eq!(store.chain()[2].name, "c");
    }
}
