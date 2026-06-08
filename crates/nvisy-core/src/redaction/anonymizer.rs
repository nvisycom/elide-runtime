//! [`Anonymizer<M>`]: the per-modality redaction operator trait.

use super::LeakProfile;
use crate::Result;
use crate::entity::Entity;
use crate::modality::Modality;

/// Per-modality redaction operator.
///
/// `apply` rewrites one entity into a [`Modality::Replacement`]. The
/// engine batches calls; per-document state (counters, caches) lives
/// on the operator instance — operators are constructed once and
/// shared across runs. Locale-aware operators (e.g. fake-data
/// generators) read `entity.language` to pick a locale.
///
/// Object-safe so registries can hold heterogeneous operators behind
/// `Arc<dyn Anonymizer<M>>`.
///
/// [`Modality::Replacement`]: crate::modality::Modality::Replacement
#[async_trait::async_trait]
pub trait Anonymizer<M: Modality>: Send + Sync {
    /// What the operator's output leaks about the original. Used by
    /// policy-authoring tools and audit reporting; not consulted in
    /// the hot path.
    fn leak_profile(&self) -> LeakProfile;

    /// Rewrite `entity` into a replacement value. `source` is the
    /// per-document payload the recognizer scanned — the operator
    /// uses it to read the original bytes the entity points at.
    async fn apply(&self, entity: &Entity<M>, source: &M::Data) -> Result<M::Replacement>;
}
