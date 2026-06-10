//! [`RedactionRegistry<M>`]: per-modality lookup tables of
//! [`Anonymizer<M>`] instances.
//!
//! The registry exposes two independent indexes plus an optional
//! catch-all:
//!
//! - **`by_kind`** — keyed by [`EntityKind`]. The dispatch the
//!   toolkit-only pipeline uses: "this entity has kind
//!   `EmailAddress`; what operator do I run?". Populated by callers
//!   with `insert_kind`.
//! - **`by_id`** — keyed by [`AnonymizerId<M>`]. The dispatch the
//!   document-side policy layer uses when a policy rule resolves to
//!   `Custom { name }` and the named operator must be looked up by
//!   string id. Populated by callers with `insert_id`.
//! - **`fallback`** — the operator to use when `by_kind.get(kind)`
//!   misses. Optional; when absent, unregistered kinds skip.
//!
//! The two indexes are independent: registering the same operator
//! both by kind and by id is a deliberate call-site choice, not an
//! automatic mirroring.
//!
//! ```ignore
//! use nvisy_core::entity::EntityKind;
//! use nvisy_core::modality::Text;
//! use nvisy_toolkit::redaction::builtin::{Mask, Redact, Replace};
//! use nvisy_toolkit::redaction::{AnonymizerId, RedactionRegistry};
//!
//! const KMS_ENCRYPT: AnonymizerId<Text> = AnonymizerId::from_static("kms_encrypt");
//!
//! let registry = RedactionRegistry::<Text>::new()
//!     .insert_kind(EntityKind::EmailAddress, Replace::new("[EMAIL]"))
//!     .insert_kind(EntityKind::PaymentCard, Mask::new('#', Some(12)))
//!     .insert_id(KMS_ENCRYPT, MyKmsOperator::new(client))
//!     .with_fallback(Redact);
//! ```
//!
//! [`Anonymizer<M>`]: super::Anonymizer

use std::collections::HashMap;
use std::sync::Arc;

use nvisy_core::Result;
use nvisy_core::entity::{Entity, EntityKind};
use nvisy_core::extraction::DataAt;
use nvisy_core::modality::Modality;
use nvisy_core::redaction::Redactions;

use super::{Anonymizer, AnonymizerId};

/// Per-modality registry of [`Anonymizer<M>`] instances, indexed by
/// both [`EntityKind`] (toolkit-side per-kind dispatch) and
/// [`AnonymizerId<M>`] (policy-side custom-operator resolution).
///
/// [`Anonymizer<M>`]: super::Anonymizer
pub struct RedactionRegistry<M: Modality> {
    by_kind: HashMap<EntityKind, Arc<dyn Anonymizer<M>>>,
    by_id: HashMap<AnonymizerId<M>, Arc<dyn Anonymizer<M>>>,
    fallback: Option<Arc<dyn Anonymizer<M>>>,
}

impl<M: Modality> RedactionRegistry<M> {
    /// Build an empty registry. Use [`insert_kind`], [`insert_id`],
    /// or [`with_fallback`] to populate it.
    ///
    /// [`insert_kind`]: Self::insert_kind
    /// [`insert_id`]: Self::insert_id
    /// [`with_fallback`]: Self::with_fallback
    #[must_use]
    pub fn new() -> Self {
        Self {
            by_kind: HashMap::new(),
            by_id: HashMap::new(),
            fallback: None,
        }
    }

    /// Register `op` as the operator the toolkit pipeline picks when
    /// it encounters an entity of `kind`. Re-registering the same
    /// kind replaces the previous instance.
    #[must_use]
    pub fn insert_kind(mut self, kind: EntityKind, op: impl Anonymizer<M> + 'static) -> Self {
        self.by_kind.insert(kind, Arc::new(op));
        self
    }

    /// Register `op` under `id` for policy-side `Custom { name }`
    /// resolution. Re-registering the same id replaces the previous
    /// instance.
    #[must_use]
    pub fn insert_id(mut self, id: AnonymizerId<M>, op: impl Anonymizer<M> + 'static) -> Self {
        self.by_id.insert(id, Arc::new(op));
        self
    }

    /// Install a catch-all operator used when [`resolve`] misses.
    /// Setting it again replaces the previous fallback.
    ///
    /// [`resolve`]: Self::resolve
    #[must_use]
    pub fn with_fallback(mut self, op: impl Anonymizer<M> + 'static) -> Self {
        self.fallback = Some(Arc::new(op));
        self
    }

    /// Resolve an entity-kind to its registered operator, falling
    /// back to the catch-all when no per-kind binding exists.
    /// Returns `None` only when neither a per-kind operator nor a
    /// fallback was registered.
    #[must_use]
    pub fn resolve(&self, kind: EntityKind) -> Option<&Arc<dyn Anonymizer<M>>> {
        self.by_kind.get(&kind).or(self.fallback.as_ref())
    }

    /// Resolve an [`AnonymizerId<M>`] to its registered operator.
    /// Used by the document-side policy layer for `Custom { name }`
    /// dispatch; does **not** consult the fallback.
    #[must_use]
    pub fn resolve_id(&self, id: &AnonymizerId<M>) -> Option<&Arc<dyn Anonymizer<M>>> {
        self.by_id.get(id)
    }

    /// Number of distinct entity-kinds registered.
    #[must_use]
    pub fn kinds_len(&self) -> usize {
        self.by_kind.len()
    }

    /// Number of distinct ids registered.
    #[must_use]
    pub fn ids_len(&self) -> usize {
        self.by_id.len()
    }

    /// `true` when neither index nor a fallback are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_kind.is_empty() && self.by_id.is_empty() && self.fallback.is_none()
    }

    /// Run [`resolve`] + [`Anonymizer::apply`] over every entity, and
    /// collect the produced replacements into a [`Redactions<M>`] batch
    /// ready to hand to a [`RedactAt<M>`] implementation.
    ///
    /// `source` is a [`DataAt<M>`] resolver: for each entity, the
    /// substring/segment **at the entity's location** is pulled from
    /// the resolver and handed to the anonymizer — the anonymizer
    /// never sees the whole document. Entities whose location can't
    /// be resolved by the source are skipped.
    ///
    /// Entities whose kind has no per-kind operator and where no
    /// fallback was registered are skipped (counted as a debug-level
    /// tracing event); the rest are applied in iteration order.
    ///
    /// [`resolve`]: Self::resolve
    /// [`Anonymizer::apply`]: super::Anonymizer::apply
    /// [`DataAt<M>`]: nvisy_core::extraction::DataAt
    /// [`RedactAt<M>`]: nvisy_core::redaction::RedactAt
    pub async fn apply_all<'a, I>(
        &self,
        entities: I,
        source: &(impl DataAt<M> + ?Sized),
    ) -> Result<Redactions<M>>
    where
        I: IntoIterator<Item = &'a Entity<M>>,
        M: 'a,
    {
        let mut out = Redactions::<M>::new();
        let mut skipped = 0usize;
        let mut unresolved = 0usize;
        for entity in entities {
            let Some(op) = self.resolve(entity.entity_kind) else {
                skipped += 1;
                continue;
            };
            let Some(data) = source.data_at(&entity.location).await else {
                unresolved += 1;
                continue;
            };
            let replacement = op.apply(entity, &data).await?;
            out.push(entity.location.clone(), replacement);
        }
        if skipped > 0 {
            tracing::debug!(
                target: "nvisy_toolkit::redaction::registry",
                skipped,
                "RedactionRegistry::apply_all skipped entities with no per-kind operator and no fallback",
            );
        }
        if unresolved > 0 {
            tracing::debug!(
                target: "nvisy_toolkit::redaction::registry",
                unresolved,
                "RedactionRegistry::apply_all skipped entities whose location resolved no source payload",
            );
        }
        Ok(out)
    }
}

impl<M: Modality> Default for RedactionRegistry<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Modality> Clone for RedactionRegistry<M> {
    fn clone(&self) -> Self {
        Self {
            by_kind: self.by_kind.clone(),
            by_id: self.by_id.clone(),
            fallback: self.fallback.clone(),
        }
    }
}

impl<M: Modality> std::fmt::Debug for RedactionRegistry<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedactionRegistry")
            .field("kinds", &self.by_kind.len())
            .field("ids", &self.by_id.len())
            .field("fallback", &self.fallback.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {

    use nvisy_core::entity::Entity;
    use nvisy_core::modality::{Text, TextData, TextLocation};
    use nvisy_core::primitive::Confidence;

    use super::*;
    use crate::redaction::{LeakProfile, TextReplacement};

    struct StubAnonymizer(&'static str);

    #[async_trait::async_trait]
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

    /// Minimal in-memory `DataAt<Text>` for tests: slices the source
    /// string at the entity's byte range, like the codec does.
    struct StubSource(String);

    #[async_trait::async_trait]
    impl DataAt<Text> for StubSource {
        async fn data_at(&self, location: &TextLocation) -> Option<TextData> {
            self.0
                .get(location.start..location.end)
                .map(|s| TextData::new(s.to_owned()))
        }
    }

    fn entity(kind: EntityKind, start: usize, end: usize) -> Entity<Text> {
        Entity::<Text>::builder()
            .with_entity_kind(kind)
            .with_location(TextLocation::new(start, end))
            .with_confidence(Confidence::new(0.9).unwrap())
            .build()
            .expect("test fixture entity must build")
    }

    #[test]
    fn empty_registry_resolves_nothing() {
        let r = RedactionRegistry::<Text>::new();
        assert!(r.is_empty());
        assert!(r.resolve(EntityKind::EmailAddress).is_none());
        assert!(r.resolve_id(&AnonymizerId::from_static("kms")).is_none());
    }

    #[test]
    fn insert_kind_then_resolve_returns_operator() {
        let r = RedactionRegistry::<Text>::new()
            .insert_kind(EntityKind::EmailAddress, StubAnonymizer("[EMAIL]"));
        assert_eq!(r.kinds_len(), 1);
        assert!(r.resolve(EntityKind::EmailAddress).is_some());
    }

    #[test]
    fn insert_id_then_resolve_id_returns_operator() {
        let id = AnonymizerId::<Text>::from_static("kms");
        let r = RedactionRegistry::<Text>::new().insert_id(id.clone(), StubAnonymizer("[KMS]"));
        assert_eq!(r.ids_len(), 1);
        assert!(r.resolve_id(&id).is_some());
    }

    #[test]
    fn fallback_covers_unregistered_kinds() {
        let r = RedactionRegistry::<Text>::new().with_fallback(StubAnonymizer("[REDACTED]"));
        assert!(r.resolve(EntityKind::PaymentCard).is_some());
    }

    #[test]
    fn per_kind_wins_over_fallback() {
        let r = RedactionRegistry::<Text>::new()
            .insert_kind(EntityKind::EmailAddress, StubAnonymizer("[EMAIL]"))
            .with_fallback(StubAnonymizer("[OTHER]"));
        // Both resolve, but per-kind takes precedence — exercised
        // indirectly via apply_all below.
        assert!(r.resolve(EntityKind::EmailAddress).is_some());
        assert!(r.resolve(EntityKind::PaymentCard).is_some());
    }

    #[tokio::test]
    async fn apply_all_uses_per_kind_with_fallback() {
        let r = RedactionRegistry::<Text>::new()
            .insert_kind(EntityKind::EmailAddress, StubAnonymizer("[EMAIL]"))
            .with_fallback(StubAnonymizer("[OTHER]"));
        let entities = [
            entity(EntityKind::EmailAddress, 0, 5),
            entity(EntityKind::PaymentCard, 6, 10),
        ];
        let source = StubSource("abcdefghij".to_owned());
        let rs = r.apply_all(entities.iter(), &source).await.unwrap();
        let items = rs.into_items();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].1, TextReplacement::substituted("[EMAIL]"));
        assert_eq!(items[1].1, TextReplacement::substituted("[OTHER]"));
    }

    #[tokio::test]
    async fn apply_all_skips_unmatched_entities_without_fallback() {
        let r = RedactionRegistry::<Text>::new()
            .insert_kind(EntityKind::EmailAddress, StubAnonymizer("[EMAIL]"));
        let entities = [
            entity(EntityKind::EmailAddress, 0, 5),
            entity(EntityKind::PaymentCard, 6, 10),
        ];
        let source = StubSource("abcdefghij".to_owned());
        let rs = r.apply_all(entities.iter(), &source).await.unwrap();
        assert_eq!(rs.len(), 1);
    }
}
