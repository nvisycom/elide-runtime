//! Core OCR contract: the [`Backend`] trait, the shared input /
//! output types, and the per-call [`Context`] hints.
//!
//! Backend implementations live in [`crate::backend`]; that module
//! also hosts the [`OcrBackend`] config enum that dispatches to a
//! concrete backend.
//!
//! [`OcrBackend`]: crate::backend::OcrBackend

mod context;
mod input;

use nvisy_core::Error;
pub use nvisy_core::content::ImageFormat;
use nvisy_ontology::document::Block;
use nvisy_ontology::entity::ModelProvenance;
use nvisy_ontology::modality::Image;

pub use self::context::Context;
pub use self::input::ImageInput;

/// The OCR backend contract.
///
/// Implementations send an image to an OCR service and return a
/// `Vec<Block<Image>>` — one block per page or text region, with
/// per-word [`Span<Image>`] populated and bounding boxes preserved
/// on each location.
///
/// Backends are source-agnostic: they take bytes + hints and return
/// shape. Wrapping the blocks into a `Document<Image>` (which
/// requires a [`ContentSource`]) happens at the engine boundary.
///
/// Confidence values **must** be normalised to `0.0..=1.0` before
/// being placed on spans. Backends whose upstream API uses a
/// different scale are responsible for converting.
///
/// Implementors **must** provide [`run`]. The default
/// [`run_batch`] impl dispatches the inputs concurrently via
/// `futures::join_all` and concatenates the per-image blocks.
/// Backends with a native batch API (such as a single network
/// round-trip) override it to merge server-side.
///
/// Batch entries share a single [`Context`] and are **assumed to
/// come from the same source** — the typical caller is a
/// multi-page document split into per-page images, so the language
/// hint applies uniformly and the per-image blocks can be merged
/// without further bookkeeping. Mixed-source inputs should be
/// issued as separate batches.
///
/// Per-image page numbering is returned as-is — if the caller
/// needs them rebased onto a containing document, the caller knows
/// the per-image page offsets and is responsible for that rebase.
///
/// [`run`]: Self::run
/// [`run_batch`]: Self::run_batch
/// [`Span<Image>`]: nvisy_ontology::document::Span
/// [`ContentSource`]: nvisy_ontology::entity::ContentSource
#[async_trait::async_trait]
pub trait Backend: Send + Sync + 'static {
    /// Backend identity (model / service name + provenance kind).
    ///
    /// The engine's [`OcrExtractor`] reads this after recognition
    /// runs and stamps it into [`ImageExtraction::Ocr(_)`] on the
    /// document's metadata, so the audit records *which* OCR pass
    /// produced the document.
    ///
    /// [`OcrExtractor`]: https://docs.rs/nvisy-engine/latest/nvisy_engine/extraction/struct.OcrExtractor.html
    /// [`ImageExtraction::Ocr(_)`]: nvisy_ontology::modality::ImageExtraction::Ocr
    fn provenance(&self) -> ModelProvenance;

    /// Run OCR on a single image under `ctx`.
    async fn run(&self, image: &ImageInput, ctx: Context<'_>) -> Result<Vec<Block<Image>>, Error>;

    /// Run OCR on each of `images` under one shared [`Context`],
    /// concatenating the per-image blocks.
    ///
    /// `images` is assumed to be slices of the same source (see
    /// the trait-level docs). The default impl dispatches
    /// concurrently via `futures::join_all` and concatenates;
    /// backends with native batching override it.
    async fn run_batch(
        &self,
        images: &[ImageInput],
        ctx: Context<'_>,
    ) -> Result<Vec<Block<Image>>, Error> {
        let pending: Vec<_> = images.iter().map(|img| self.run(img, ctx)).collect();
        let results: Vec<Result<Vec<Block<Image>>, Error>> =
            futures::future::join_all(pending).await;
        let mut merged: Vec<Block<Image>> = Vec::new();
        for r in results {
            merged.extend(r?);
        }
        Ok(merged)
    }
}
