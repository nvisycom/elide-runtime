//! [`RedactAt<M>`]: write-back sibling of [`DataAt`] / [`TextAt`].
//!
//! Where [`DataAt`] reads a typed payload at a per-modality location
//! and [`TextAt`] reads the display text at it, `RedactAt` writes
//! per-modality replacements back into the underlying source.
//!
//! Implementations include the codec layer's typed
//! `DocumentHandle<M>` (writes into the format-specific bytes) and
//! the engine's `DocumentTree<M>` (forwards through the codec
//! handle).
//!
//! The trait takes a batched [`Redactions<M>`] because most
//! implementations need ordering control (right-to-left for text /
//! audio so earlier shifts don't invalidate later coordinates,
//! batched per page for PDF, …).
//!
//! [`DataAt`]: crate::extraction::DataAt
//! [`TextAt`]: crate::extraction::TextAt

use crate::Result;
use crate::modality::Modality;
use crate::redaction::Redactions;

/// Apply a batch of `(location, replacement)` pairs to a per-modality
/// source. Producer guarantees non-overlapping locations; the
/// implementation reorders the batch as needed.
#[async_trait::async_trait]
pub trait RedactAt<M: Modality>: Send + Sync {
    /// Apply the batch in whatever order is correct for this
    /// implementation. The first error aborts the batch.
    async fn redact_at(&mut self, redactions: Redactions<M>) -> Result<()>;
}
