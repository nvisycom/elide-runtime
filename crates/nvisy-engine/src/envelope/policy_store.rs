//! [`PolicyStore`]: heterogeneous container of [`Policy<M>`] keyed by
//! modality, backed by a [`TypeMap`].
//!
//! `Policy<M>` is generic over its modality; engine state ([`SharedData`])
//! needs to hold policies for any modality without exposing a generic
//! surface or a fixed per-modality field set. `PolicyStore` provides
//! a single uniform container with typed `insert`/`get`/`len`
//! accessors parameterised over `M`.
//!
//! Internally one `Vec<Policy<M>>` is stored per modality; lookups
//! cost a single `TypeId` hash.
//!
//! [`SharedData`]: super::SharedData

use nvisy_ontology::modality::Modality;
use nvisy_ontology::policy::Policy;
use type_map::concurrent::TypeMap;

/// Heterogeneous container of [`Policy<M>`] across all modalities.
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
    /// preserved (callers feed policies in precedence order).
    pub fn insert<M: Modality>(&mut self, policy: Policy<M>) {
        self.bucket_mut::<M>().push(policy);
    }

    /// Replace the policy stack for modality `M`.
    pub fn set<M: Modality>(&mut self, policies: Vec<Policy<M>>) {
        self.inner.insert::<Vec<Policy<M>>>(policies);
    }

    /// Borrow the policy stack for modality `M`. Returns an empty
    /// slice when no policies of that modality have been inserted.
    pub fn get<M: Modality>(&self) -> &[Policy<M>] {
        self.inner
            .get::<Vec<Policy<M>>>()
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Number of policies stored for modality `M`.
    pub fn len<M: Modality>(&self) -> usize {
        self.get::<M>().len()
    }

    /// `true` when no policies for modality `M` are stored.
    pub fn is_empty<M: Modality>(&self) -> bool {
        self.get::<M>().is_empty()
    }

    fn bucket_mut<M: Modality>(&mut self) -> &mut Vec<Policy<M>> {
        self.inner.entry::<Vec<Policy<M>>>().or_insert_with(Vec::new)
    }
}

impl std::fmt::Debug for PolicyStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolicyStore").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::modality::{Image, Text};
    use semver::Version;

    use super::*;

    fn text_policy() -> Policy<Text> {
        Policy::<Text> {
            id: uuid::Uuid::nil(),
            name: "test".into(),
            version: Version::new(1, 0, 0),
            description: None,
            default_strategy: None,
            strategies: Vec::new(),
            retention: Vec::new(),
        }
    }

    fn image_policy() -> Policy<Image> {
        Policy::<Image> {
            id: uuid::Uuid::nil(),
            name: "test".into(),
            version: Version::new(1, 0, 0),
            description: None,
            default_strategy: None,
            strategies: Vec::new(),
            retention: Vec::new(),
        }
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
}
