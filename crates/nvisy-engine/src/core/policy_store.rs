//! [`PolicyStore`]: heterogeneous container of [`Policy<M>`] keyed by
//! modality, backed by a [`TypeMap`], plus the per-entity decision
//! resolver that walks it.
//!
//! Built from a `Vec<AnyPolicy>` submission via
//! [`PolicyStore::from_any_policies`], which consumes the submitted
//! policies and wraps each in an `Arc<Policy<M>>` — no deep clones.
//! Detection and redaction pipelines share a single store via
//! `Arc<PolicyStore>`; per-call handoff is a refcount bump.
//!
//! Internally one `Vec<Arc<Policy<M>>>` is stored per modality;
//! lookups cost a single `TypeId` hash. The only crate-public
//! operation is [`PolicyStore::resolve`].
//!
//! [`SharedData`]: super::SharedData

use std::sync::Arc;

use hipstr::HipStr;
use nvisy_codec::content::ContentDescriptor;
use nvisy_core::entity::Entity;
use type_map::concurrent::TypeMap;

use crate::modality::DocumentModality;
use crate::policy::{Action, AnyPolicy, Condition, Policy, PolicyRule};

/// Heterogeneous container of policies across all modalities,
/// stored as `Arc<Policy<M>>` so handoff between detection and
/// redaction pipelines is a refcount bump rather than a deep clone.
///
/// # Type-safe per-modality storage
///
/// Backed by `type_map::TypeMap`, which stores at most one value
/// per concrete type. The crate-internal `resolve::<M>` method
/// looks the right bucket up by `TypeId`, so adding a new modality
/// is purely an `AnyPolicy::NewM(...)` arm in the crate-internal
/// constructor — no hardcoded fields or per-modality methods to
/// maintain.
#[derive(Default)]
pub struct PolicyStore {
    inner: TypeMap,
}

impl PolicyStore {
    /// Construct a store from a `Vec<AnyPolicy>` submission, taking
    /// ownership of the policies (so each [`Policy<M>`] is moved
    /// straight into its [`Arc`] — no deep clone).
    pub(crate) fn from_any_policies(policies: Vec<AnyPolicy>) -> Self {
        use crate::modality::{Audio, Image, Tabular, Text};

        let mut store = Self::default();
        for any in policies {
            match any {
                AnyPolicy::Text(p) => store.push::<Text>(Arc::new(p)),
                AnyPolicy::Tabular(p) => store.push::<Tabular>(Arc::new(p)),
                AnyPolicy::Image(p) => store.push::<Image>(Arc::new(p)),
                AnyPolicy::Audio(p) => store.push::<Audio>(Arc::new(p)),
            }
        }
        store
    }

    fn push<M: DocumentModality>(&mut self, policy: Arc<Policy<M>>) {
        self.bucket_mut::<M>().push(policy);
    }

    /// Union every stored policy's [`Policy::labels`] into a single
    /// [`EntityLabelCatalog`]. Used at redaction time to rebuild the
    /// per-request catalog the detection pass already validated.
    /// Conflicts here are impossible because the same union was
    /// validated at detection-time submission.
    pub(crate) fn catalog(&self) -> nvisy_core::entity::EntityLabelCatalog {
        use crate::modality::{Audio, Image, Tabular, Text};

        let mut catalog = nvisy_core::entity::EntityLabelCatalog::new();
        for p in self.chain::<Text>() {
            for l in &p.labels {
                catalog.insert(l.clone());
            }
        }
        for p in self.chain::<Tabular>() {
            for l in &p.labels {
                catalog.insert(l.clone());
            }
        }
        for p in self.chain::<Image>() {
            for l in &p.labels {
                catalog.insert(l.clone());
            }
        }
        for p in self.chain::<Audio>() {
            for l in &p.labels {
                catalog.insert(l.clone());
            }
        }
        catalog
    }

    fn chain<M: DocumentModality>(&self) -> &[Arc<Policy<M>>] {
        self.inner
            .get::<Vec<Arc<Policy<M>>>>()
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Resolve a single entity against the per-modality policy
    /// chain. Walks policies in precedence order; within a policy,
    /// walks rules in declaration order. First matching rule wins;
    /// when no rule in a policy matches, falls back to that policy's
    /// [`Policy::default_action`] (if any) before descending to the
    /// next policy.
    ///
    /// Returns [`Decision::Fallthrough`] when no policy in the
    /// chain produced a decision; the caller's default-threshold
    /// path takes over.
    pub(crate) fn resolve<M: DocumentModality>(
        &self,
        entity: &Entity<M>,
        catalog: &nvisy_core::entity::EntityLabelCatalog,
        document_labels: &[&str],
        descriptor: &ContentDescriptor,
    ) -> Decision<M> {
        for policy in self.chain::<M>() {
            for rule in &policy.rules {
                if !rule_matches(rule, entity, catalog, document_labels, descriptor) {
                    continue;
                }
                let policy_name = policy.name.clone();
                let rule_name = Some(rule.name.clone());
                return match &rule.action {
                    Action::Redact { operator } => Decision::Redact {
                        policy_name,
                        rule_name,
                        operator: operator.clone(),
                    },
                    Action::Suppress => Decision::Suppress {
                        policy_name,
                        rule_name,
                    },
                };
            }
            if let Some(default) = policy.default_action.as_ref() {
                let policy_name = policy.name.clone();
                return match default {
                    Action::Redact { operator } => Decision::Redact {
                        policy_name,
                        rule_name: None,
                        operator: operator.clone(),
                    },
                    Action::Suppress => Decision::Suppress {
                        policy_name,
                        rule_name: None,
                    },
                };
            }
        }
        Decision::Fallthrough
    }

    fn bucket_mut<M: DocumentModality>(&mut self) -> &mut Vec<Arc<Policy<M>>> {
        self.inner
            .entry::<Vec<Arc<Policy<M>>>>()
            .or_insert_with(Vec::new)
    }
}

impl std::fmt::Debug for PolicyStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolicyStore").finish_non_exhaustive()
    }
}

/// Outcome of walking a per-modality policy chain for one entity.
///
/// The winning rule's operator spec ([`DocumentModality::Redaction`])
/// rides along on `Decision::Redact` so the apply phase can
/// instantiate the operator (built-in arms) or look it up in the
/// toolkit [`RedactionRegistry<M>`] (`Custom` arm) without re-walking
/// the policy chain.
///
/// [`DocumentModality::Redaction`]: crate::modality::DocumentModality::Redaction
/// [`RedactionRegistry<M>`]: nvisy_toolkit::redaction::RedactionRegistry
pub(crate) enum Decision<M: DocumentModality> {
    /// A rule chose to redact. `operator` is the per-modality
    /// operator spec the winning rule carried; `policy_name` +
    /// `rule_name` locate the producing rule. `rule_name` is `None`
    /// when the policy's `default_action` fallback fired.
    Redact {
        policy_name: HipStr<'static>,
        rule_name: Option<HipStr<'static>>,
        operator: M::Redaction,
    },
    /// A `Suppress` rule fired; the caller records the suppression.
    /// Same naming semantics as [`Decision::Redact`].
    Suppress {
        policy_name: HipStr<'static>,
        rule_name: Option<HipStr<'static>>,
    },
    /// No policy in the chain produced a decision. The caller falls
    /// back to its default-threshold path.
    Fallthrough,
}

fn rule_matches<M: DocumentModality>(
    rule: &PolicyRule<M>,
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
    use nvisy_core::modality::{Image, Text};
    use semver::Version;

    use super::*;

    fn text_policy(name: &str) -> Policy<Text> {
        Policy::<Text> {
            name: HipStr::from(name),
            version: Version::new(1, 0, 0),
            description: None,
            labels: Vec::new(),
            rules: Vec::new(),
            default_action: None,
            retention: Vec::new(),
        }
    }

    fn image_policy(name: &str) -> Policy<Image> {
        Policy::<Image> {
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
        assert!(matches!(
            store.resolve::<Text>(&entity, &catalog, &[], &descriptor),
            Decision::Fallthrough
        ));
    }

    #[test]
    fn from_any_policies_partitions_by_modality() {
        let store = PolicyStore::from_any_policies(vec![
            AnyPolicy::Text(text_policy("text-1")),
            AnyPolicy::Image(image_policy("image-1")),
            AnyPolicy::Text(text_policy("text-2")),
        ]);
        assert_eq!(store.chain::<Text>().len(), 2);
        assert_eq!(store.chain::<Image>().len(), 1);
    }
}
