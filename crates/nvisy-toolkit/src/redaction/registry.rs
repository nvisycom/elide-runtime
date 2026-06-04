//! [`RedactionRegistry<M>`]: per-modality lookup table of custom
//! [`Anonymizer<M>`] instances, keyed by [`AnonymizerId<M>`].
//!
//! Built once at startup by deployment code. Holds *only* the custom
//! operators users plug in to extend the closed set of built-ins —
//! the built-in operators ([`Replace`][r], [`Mask`][m], …) are
//! instantiated per-call from the policy's operator-spec enum and
//! never live here.
//!
//! ```ignore
//! use nvisy_toolkit::redaction::{AnonymizerId, RedactionRegistry};
//! use nvisy_core::modality::Text;
//!
//! const KMS_ENCRYPT: AnonymizerId<Text> = AnonymizerId::from_static("kms_encrypt");
//!
//! let registry = RedactionRegistry::<Text>::new()
//!     .insert(KMS_ENCRYPT, MyKmsOperator::new(client));
//! ```
//!
//! [`Anonymizer<M>`]: super::Anonymizer
//! [r]: super::builtin::Replace
//! [m]: super::builtin::Mask

use std::collections::HashMap;
use std::sync::Arc;

use nvisy_core::modality::ModalityData;

use super::{Anonymizer, AnonymizerId, Redactable};

/// Per-modality registry of custom [`Anonymizer<M>`] instances.
///
/// Lookup is by [`AnonymizerId<M>`]; the phantom on the id guarantees
/// no cross-modality misuse at the call site.
///
/// [`Anonymizer<M>`]: super::Anonymizer
pub struct RedactionRegistry<M: Redactable + ModalityData> {
    inner: HashMap<AnonymizerId<M>, Arc<dyn Anonymizer<M>>>,
}

impl<M: Redactable + ModalityData> RedactionRegistry<M> {
    /// Build an empty registry. Use [`insert`][Self::insert] to
    /// register custom operators.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    /// Register a custom operator under `id`. Re-registering the
    /// same id replaces the previous instance.
    #[must_use]
    pub fn insert(mut self, id: AnonymizerId<M>, op: impl Anonymizer<M> + 'static) -> Self {
        self.inner.insert(id, Arc::new(op));
        self
    }

    /// Look up a registered operator by id.
    #[must_use]
    pub fn get(&self, id: &AnonymizerId<M>) -> Option<&Arc<dyn Anonymizer<M>>> {
        self.inner.get(id)
    }

    /// `true` when no operators are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Number of registered operators.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<M: Redactable + ModalityData> Default for RedactionRegistry<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Redactable + ModalityData> Clone for RedactionRegistry<M> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<M: Redactable + ModalityData> std::fmt::Debug for RedactionRegistry<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedactionRegistry")
            .field("len", &self.inner.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use nvisy_core::Result;
    use nvisy_core::entity::Entity;
    use nvisy_core::modality::{Text, TextData};

    use super::*;
    use crate::redaction::{LeakProfile, TextReplacement};

    struct StubAnonymizer(&'static str);

    #[async_trait]
    impl Anonymizer<Text> for StubAnonymizer {
        fn leak_profile(&self) -> LeakProfile {
            LeakProfile::Irrecoverable
        }

        async fn apply(
            &self,
            _entity: &Entity<Text>,
            _source: &TextData,
        ) -> Result<TextReplacement> {
            Ok(TextReplacement::substituted(self.0))
        }
    }

    #[test]
    fn empty_registry_returns_none() {
        let r = RedactionRegistry::<Text>::new();
        assert!(r.is_empty());
        assert!(r.get(&AnonymizerId::from_static("kms")).is_none());
    }

    #[test]
    fn insert_then_get_returns_operator() {
        let id = AnonymizerId::<Text>::from_static("stub");
        let r = RedactionRegistry::<Text>::new().insert(id.clone(), StubAnonymizer("a"));
        assert_eq!(r.len(), 1);
        assert!(r.get(&id).is_some());
    }

    #[test]
    fn re_inserting_same_id_replaces() {
        let id = AnonymizerId::<Text>::from_static("stub");
        let r = RedactionRegistry::<Text>::new()
            .insert(id.clone(), StubAnonymizer("a"))
            .insert(id.clone(), StubAnonymizer("b"));
        assert_eq!(r.len(), 1);
        assert!(r.get(&id).is_some());
    }
}
