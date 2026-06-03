//! [`PolicyStore`]: heterogeneous container of [`Policy<M>`] keyed by
//! modality, backed by a [`TypeMap`], plus the per-entity decision
//! resolver that walks it.
//!
//! `Policy<M>` is generic over its modality; engine state ([`SharedData`])
//! needs to hold policies for any modality without exposing a generic
//! surface or a fixed per-modality field set. `PolicyStore` provides
//! a single uniform container with typed `insert`/`get`/`len`
//! accessors parameterised over `M`, and a [`PolicyStore::resolve`]
//! method that walks the per-modality chain to pick a [`Decision`]
//! for a single entity.
//!
//! Internally one `Vec<Policy<M>>` is stored per modality; lookups
//! cost a single `TypeId` hash.
//!
//! [`SharedData`]: super::SharedData

use std::sync::Arc;

use nvisy_core::content::ContentMetadata;
use nvisy_core::entity::Entity;
use nvisy_toolkit::redaction::Redactable;
use type_map::concurrent::TypeMap;
use uuid::Uuid;

use crate::modality::DocumentModality;
use crate::policy::{Action, Condition, Policy, PolicyRule, RuleRank};

/// Heterogeneous container of policies across all modalities,
/// stored as `Arc<Policy<M>>` so that multiple per-run stores can
/// share the same loaded policy instances cheaply (the registry's
/// cross-run cache hands out `Arc<Policy<M>>` clones).
///
/// # Type-safe per-modality storage
///
/// Backed by `type_map::TypeMap`, which stores at most one value
/// per concrete type. The accessors are parameterised over `M`, so
/// `insert::<Text>(...)` and `insert::<Image>(...)` go into
/// independent buckets; `get::<Text>()` is statically guaranteed
/// to return text policies — there is no way to retrieve an
/// `Image` bucket through a `Text` type parameter. The compiler
/// rejects mismatched-modality calls at the call site, not at
/// runtime.
#[derive(Default)]
pub struct PolicyStore {
    inner: TypeMap,
}

impl PolicyStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a policy for modality `M`. Order within a modality is
    /// preserved (callers feed policies in precedence order). The
    /// policy is held by [`Arc`] so it can also live in the
    /// registry's cross-run cache without copying.
    pub fn insert<M: DocumentModality + Redactable>(&mut self, policy: Arc<Policy<M>>) {
        self.bucket_mut::<M>().push(policy);
    }

    /// Replace the policy stack for modality `M`.
    pub fn set<M: DocumentModality + Redactable>(&mut self, policies: Vec<Arc<Policy<M>>>) {
        self.inner.insert::<Vec<Arc<Policy<M>>>>(policies);
    }

    /// Borrow the policy stack for modality `M`. Returns an empty
    /// slice when no policies of that modality have been inserted.
    /// Each element is an `Arc<Policy<M>>` — deref through it to
    /// read fields.
    pub fn get<M: DocumentModality + Redactable>(&self) -> &[Arc<Policy<M>>] {
        self.inner
            .get::<Vec<Arc<Policy<M>>>>()
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Number of policies stored for modality `M`.
    pub fn len<M: DocumentModality + Redactable>(&self) -> usize {
        self.get::<M>().len()
    }

    /// `true` when no policies for modality `M` are stored.
    pub fn is_empty<M: DocumentModality + Redactable>(&self) -> bool {
        self.get::<M>().is_empty()
    }

    /// Resolve a single entity against the per-modality policy
    /// chain. Walks layers in precedence order; within a layer,
    /// walks rules in declaration order. First matching rule wins;
    /// when no rule in a layer matches, falls back to that layer's
    /// [`Policy::default_strategy`] (if any) before descending to
    /// the next layer.
    ///
    /// Returns [`Decision::Fallthrough`] when no policy in the
    /// chain produced a decision; the caller's default-threshold
    /// path takes over. Crate-internal — the evaluator in
    /// `redaction::evaluate` is the only caller.
    pub(crate) fn resolve<M: DocumentModality + Redactable>(
        &self,
        entity: &Entity<M>,
        document_labels: &[&str],
        metadata: &ContentMetadata,
    ) -> Decision<M> {
        for (policy_idx, policy) in self.get::<M>().iter().enumerate() {
            let policy_index = u32::try_from(policy_idx).unwrap_or(u32::MAX);
            for (rule_idx, rule) in policy.rules.iter().enumerate() {
                if !rule_matches(rule, entity, document_labels, metadata) {
                    continue;
                }
                let rule_index = u32::try_from(rule_idx).unwrap_or(u32::MAX);
                let rank = RuleRank::new(policy_index, rule_index);
                return match &rule.action {
                    Action::Redact { strategy } => Decision::Redact {
                        strategy: strategy.clone(),
                        policy_id: policy.id,
                        rank,
                    },
                    Action::Suppress => Decision::Suppress {
                        policy_id: policy.id,
                        rank,
                    },
                };
            }
            if let Some(default) = policy.default_strategy.clone() {
                return Decision::Redact {
                    strategy: default,
                    policy_id: policy.id,
                    rank: RuleRank::for_default(policy_index),
                };
            }
        }
        Decision::Fallthrough
    }

    fn bucket_mut<M: DocumentModality + Redactable>(&mut self) -> &mut Vec<Arc<Policy<M>>> {
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
pub(crate) enum Decision<M: DocumentModality + Redactable> {
    /// A rule chose a strategy. `rank` locates the producing rule
    /// inside the chain for codec-side tiebreaking.
    Redact {
        strategy: M::Strategy,
        policy_id: Uuid,
        rank: RuleRank,
    },
    /// A `Suppress` rule fired; the caller records the suppression.
    Suppress { policy_id: Uuid, rank: RuleRank },
    /// No policy in the chain produced a decision. The caller falls
    /// back to its default-threshold path.
    Fallthrough,
}

fn rule_matches<M: DocumentModality + Redactable>(
    rule: &PolicyRule<M>,
    entity: &Entity<M>,
    document_labels: &[&str],
    metadata: &ContentMetadata,
) -> bool {
    rule.enabled
        && rule.selector.matches(entity)
        && rule
            .conditions
            .iter()
            .all(|c| condition_matches(c, document_labels, metadata))
}

fn condition_matches(
    condition: &Condition,
    document_labels: &[&str],
    metadata: &ContentMetadata,
) -> bool {
    match condition {
        Condition::Labels { labels } => labels.iter().all(|label| {
            document_labels
                .iter()
                .any(|doc| doc.eq_ignore_ascii_case(label))
        }),
        Condition::Metadata { key, value } => match metadata.get_extra(key) {
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

    fn text_policy() -> Arc<Policy<Text>> {
        Arc::new(Policy::<Text> {
            id: uuid::Uuid::nil(),
            name: "test".into(),
            version: Version::new(1, 0, 0),
            description: None,
            rules: Vec::new(),
            default_strategy: None,
            retention: Vec::new(),
        })
    }

    fn image_policy() -> Arc<Policy<Image>> {
        Arc::new(Policy::<Image> {
            id: uuid::Uuid::nil(),
            name: "test".into(),
            version: Version::new(1, 0, 0),
            description: None,
            rules: Vec::new(),
            default_strategy: None,
            retention: Vec::new(),
        })
    }

    #[test]
    fn empty_store_returns_empty_slice() {
        let store = PolicyStore::new();
        assert!(store.get::<Text>().is_empty());
        assert_eq!(store.len::<Text>(), 0);
    }

    #[test]
    fn insert_and_get_per_modality() {
        let mut store = PolicyStore::new();
        store.insert(text_policy());
        store.insert(image_policy());
        store.insert(text_policy());
        assert_eq!(store.len::<Text>(), 2);
        assert_eq!(store.len::<Image>(), 1);
    }

    #[test]
    fn set_replaces_bucket() {
        let mut store = PolicyStore::new();
        store.insert(text_policy());
        store.set::<Text>(vec![text_policy(), text_policy(), text_policy()]);
        assert_eq!(store.len::<Text>(), 3);
    }

    #[test]
    fn resolve_empty_chain_returns_fallthrough() {
        let store = PolicyStore::new();
        let entity = Entity::<Text>::test_builder(0, 4).test_build();
        let metadata = ContentMetadata::new();
        assert!(matches!(
            store.resolve::<Text>(&entity, &[], &metadata),
            Decision::Fallthrough
        ));
    }
}
