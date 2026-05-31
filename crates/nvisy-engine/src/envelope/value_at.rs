//! [`ValueAt<M>`]: per-modality source-text lookup.
//!
//! Each modality resolves a location differently: text/tabular read
//! the bytes from the codec handle, while image/audio walk the
//! extraction document's blocks for a matching region or span.
//! Generic engine code (deduplication, fusion, redaction) bounds
//! over `PhaseTarget<'_, M>: ValueAt<M>` and calls into the right
//! implementation at the call site.
//!
//! The trait impls live on [`PhaseTarget<'_, M>`] in
//! `pipeline::target`; the trait definition stays here so the
//! envelope module (the historical owner of value lookup) re-exports
//! it for compatibility with existing call sites.
//!
//! [`PhaseTarget<'_, M>`]: crate::pipeline::PhaseTarget

use nvisy_ontology::modality::Modality;

/// Resolve a location of modality `M` to the corresponding source
/// text, for any per-call surface that knows how to look it up.
#[async_trait::async_trait]
pub trait ValueAt<M: Modality>: Sync {
    /// Resolve a location to its source text representation, or
    /// `None` if the underlying handle / extraction document has
    /// nothing at that location.
    async fn value_at(&self, location: &M) -> Option<String>;
}
