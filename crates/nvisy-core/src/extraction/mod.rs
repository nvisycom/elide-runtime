//! Extraction-side primitives shared across the runtime.
//!
//! - [`Extractor<M>`] — the per-modality extraction trait every
//!   backend implements. Symmetric to [`EntityRecognizer`] on the
//!   producer side but uses its own input / output shapes.
//! - [`Span<M>`] — per-call input: payload + per-modality location +
//!   optional language / correlation id + typed [`Artifacts`].
//! - [`ExtractorOutput<M, T>`] — paired return shape: the
//!   backend-shaped `value` plus the per-modality provenance value
//!   the document stamps.
//! - [`Artifacts`] — heterogeneous typed-map newtype attached to a
//!   [`Span`] so extractors carry out-of-band enrichments alongside
//!   the payload.
//! - [`ModalityExtraction`] — extension trait naming
//!   `M::Extraction`.
//! - [`TextAt`] — trait every extraction-aware consumer (dedup
//!   layer, validation check) bounds on to read source *text* at a
//!   per-modality location.
//! - [`DataAt`] — sibling trait returning the full per-modality
//!   [`M::Data`] payload an `Anonymizer<M>` operates on; bounded
//!   by the redaction phase.
//!
//! [`M::Data`]: crate::modality::ModalityData::Data
//! [`EntityRecognizer`]: crate::EntityRecognizer

mod artifacts;
mod data_at;
mod modality;
mod output;
mod span;
mod text_at;

pub use self::artifacts::Artifacts;
pub use self::data_at::DataAt;
pub use self::modality::ModalityExtraction;
pub use self::output::ExtractorOutput;
pub use self::span::Span;
pub use self::text_at::TextAt;
use crate::Result;
use crate::modality::ModalityData;

/// Per-modality extractor: convert a per-call [`Span<M>`] into a
/// backend-shaped `value` plus the modality-keyed provenance the
/// document stamps at extraction time.
///
/// Object-safe so heterogeneous extractors live behind
/// `Arc<dyn Extractor<M, Output = O>>` in consumer-side registries.
#[async_trait::async_trait]
pub trait Extractor<M>: Send + Sync
where
    M: ModalityData + ModalityExtraction,
{
    /// The extractor's modality-specific return shape. Pick whatever
    /// the underlying backend naturally produces; consumer glue
    /// translates it into per-document [`Block<M>`] values.
    ///
    /// [`Block<M>`]: # "carrier owned by nvisy-document"
    type Output: Send;

    /// Extract from `span`, returning the modality-specific output
    /// alongside the provenance value the consumer stamps into the
    /// document's metadata.
    async fn extract(&self, span: &Span<M>) -> Result<ExtractorOutput<M, Self::Output>>;
}
