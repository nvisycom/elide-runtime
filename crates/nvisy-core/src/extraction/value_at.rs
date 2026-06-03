//! [`ValueAt`]: resolve a per-modality location to its source text.
//!
//! The toolkit-side phase code (deduplication layers, validation
//! checks, redaction strategy bindings) all need to read back the
//! original text at a modality-typed location. The trait lives here
//! in core because every layer of the stack — toolkit components,
//! document phase drivers, custom user pipelines — bounds on it.
//!
//! Concrete implementations live where the underlying resolver lives:
//! `nvisy-document` ships a `DocumentView<'_, M>` impl that consults
//! the codec handle for text/tabular and the document blocks for
//! image/audio.

use crate::modality::Modality;

/// Resolve a modality-typed location to the corresponding source
/// text. Generic per-phase code bounds over `&impl ValueAt<M>` and
/// dispatches uniformly across modalities.
#[async_trait::async_trait]
pub trait ValueAt<M: Modality>: Sync {
    /// Resolve a location to its source-text representation, or
    /// `None` when no readable text exists at the location.
    async fn value_at(&self, location: &M::Location) -> Option<String>;
}
