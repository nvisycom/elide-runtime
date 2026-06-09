//! [`DataAt`]: resolve a per-modality location to its full
//! [`M::Data`] payload — the same shape an [`Anonymizer<M>`] needs.
//!
//! Sibling to [`TextAt`]: where `TextAt` answers "what display text
//! represents this location" (the read-only string a dedup layer or
//! validation check inspects), `DataAt` answers "what is the full
//! typed payload at this location" so a redaction operator can run
//! against it. The two intentionally have different return shapes —
//! `TextAt` lets consumers stay text-agnostic across modalities;
//! `DataAt` preserves the modality-specific [`M::Data`] envelope
//! the operator was written against.
//!
//! Concrete implementations live where the underlying resolver
//! lives: `nvisy-engine` ships impls on `DocumentTree<M>` that
//! consult the codec handle's `read` method.
//!
//! [`TextAt`]: super::TextAt
//! [`Anonymizer<M>`]: # "lives in nvisy-toolkit"
//! [`M::Data`]: crate::modality::Modality::Data

use crate::modality::Modality;

/// Resolve a modality-typed location to its full [`M::Data`]
/// payload. Generic per-phase code bounds over `&impl DataAt<M>`
/// and dispatches uniformly across modalities.
///
/// [`M::Data`]: Modality::Data
#[async_trait::async_trait]
pub trait DataAt<M: Modality>: Sync {
    /// Resolve a location to its typed source payload, or `None`
    /// when no payload exists at the location (out-of-bounds, the
    /// handle doesn't expose that modality, …).
    async fn data_at(&self, location: &M::Location) -> Option<M::Data>;
}
