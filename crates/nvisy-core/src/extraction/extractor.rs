//! [`Extractor<M>`]: the per-modality extraction contract.
//!
//! Symmetric to [`EntityRecognizer<M>`] — the input is dictated by the
//! modality (via [`ModalityData::Data`]), the output is dictated by
//! the extractor (via the [`Output`] associated type), and the value
//! stamped into the document's provenance trail is dictated by the
//! modality (via [`ModalityExtraction::Extraction`]).
//!
//! Each per-modality extractor produces its own output shape — OCR
//! yields region-bearing image regions, STT yields a transcript.
//! `nvisy-core` doesn't try to unify the output shapes; downstream
//! glue (document phase code, custom drivers) maps each extractor's
//! `Output` into the document's `Block<M>` values.
//!
//! [`EntityRecognizer<M>`]: crate::EntityRecognizer
//! [`ModalityData::Data`]: crate::ModalityData::Data
//! [`ModalityExtraction::Extraction`]: crate::modality::ModalityExtraction::Extraction
//! [`Output`]: Extractor::Output

use async_trait::async_trait;

use crate::Result;
use crate::modality::ModalityExtraction;
use crate::recognition::{ModalityData, RecognizerInput};

/// Per-modality extractor: convert a per-call payload into one
/// extractor-shaped output, plus the modality-keyed provenance value
/// the document stamps at extraction time.
///
/// Implementors pick an [`Output`] shape that suits the underlying
/// backend's natural return type. Object-safe so heterogeneous
/// extractors live behind `Arc<dyn Extractor<M, Output = O>>` in
/// consumer-side registries.
///
/// [`Output`]: Self::Output
#[async_trait]
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

    /// The per-modality extraction provenance value the consumer
    /// stamps into the document's metadata at extraction time.
    fn extraction(&self) -> M::Extraction;

    /// Extract from `input`, returning the modality-specific output.
    async fn extract(&self, input: &RecognizerInput<M>) -> Result<Self::Output>;
}
