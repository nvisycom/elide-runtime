//! [`Anonymizer<M>`]: the per-modality redaction operator trait.
//!
//! Each operator is a typed Rust struct (no name strings, no
//! parameter dicts). The struct's fields carry the operator's
//! parameters (mask character, hash algorithm, encryption key id,
//! …); `apply` runs the transformation once per entity. Object-safe
//! so the [`RedactionRegistry`] can hold heterogeneous operators
//! behind `Arc<dyn Anonymizer<M>>`.
//!
//! [`RedactionRegistry`]: super::RedactionRegistry

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_core::entity::Entity;
use nvisy_core::modality::ModalityData;

use super::{LeakProfile, Redactable};

/// Per-modality redaction operator.
///
/// `apply` rewrites one entity into a [`M::Replacement`]. The
/// engine batches calls; per-document state (counters, caches) lives
/// on the operator instance — operators are constructed once and
/// shared across runs.
///
/// The [`ModalityData`] bound on `M` lets `apply` borrow the source
/// payload the recognizer scanned. Modalities without a payload type
/// (e.g. `Tabular` today) can still implement [`Redactable`] for use
/// in policy / audit type signatures; they just can't have an
/// `Anonymizer` impl yet.
///
/// [`M::Replacement`]: Redactable::Replacement
#[async_trait]
pub trait Anonymizer<M: Redactable + ModalityData>: Send + Sync {
    /// What the operator's output leaks about the original. Used by
    /// policy-authoring tools and audit reporting; not consulted in
    /// the hot path.
    fn leak_profile(&self) -> LeakProfile;

    /// Rewrite `entity` into a replacement value. `source` is the
    /// per-document payload the recognizer scanned — the operator
    /// uses it to read the original bytes the entity points at.
    async fn apply(&self, entity: &Entity<M>, source: &M::Data) -> Result<M::Replacement>;
}
