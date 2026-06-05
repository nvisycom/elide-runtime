//! [`SourceAt`]: resolve a per-modality location to its full
//! [`M::Data`] payload — the same shape an [`Anonymizer<M>`] needs.
//!
//! Sibling to [`ValueAt`]: where `ValueAt` answers "what text
//! lives at this location" (the read-only string a dedup layer or
//! validation check inspects), `SourceAt` answers "what is the full
//! typed payload at this location" so a redaction operator can run
//! against it. The two intentionally have different return shapes —
//! `ValueAt` lets consumers stay text-agnostic across modalities;
//! `SourceAt` preserves the modality-specific [`M::Data`] envelope
//! the operator was written against.
//!
//! Concrete implementations live where the underlying resolver
//! lives: `nvisy-document` ships a `HandleSource<'_>` adapter that
//! consults the codec handle's modality-specific `read_*` methods.
//!
//! [`ValueAt`]: super::ValueAt
//! [`Anonymizer<M>`]: # "lives in nvisy-toolkit"
//! [`M::Data`]: crate::modality::ModalityData::Data

use crate::modality::ModalityData;

/// Resolve a modality-typed location to its full [`M::Data`]
/// payload. Generic per-phase code bounds over `&impl SourceAt<M>`
/// and dispatches uniformly across modalities.
///
/// [`M::Data`]: ModalityData::Data
#[async_trait::async_trait]
pub trait SourceAt<M: ModalityData>: Sync {
    /// Resolve a location to its typed source payload, or `None`
    /// when no payload exists at the location (out-of-bounds, the
    /// handle doesn't expose that modality, …).
    async fn source_at(&self, location: &M::Location) -> Option<M::Data>;
}
