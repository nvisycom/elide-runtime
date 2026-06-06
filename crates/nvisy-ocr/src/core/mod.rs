//! Core OCR contract: the [`Backend`] trait, the shared input /
//! output types, and the per-call [`Context`] hints.
//!
//! Backend implementations live in [`crate::backend`]. The
//! TOML-deserialisable selector that picks one (`OcrBackend`) lives
//! in the pipeline layer (`nvisy-document`'s
//! `pipeline::config::extraction::ocr`).

mod context;
mod input;
mod output;

use nvisy_core::Error;
use nvisy_core::entity::ModelProvenance;

pub use self::context::Context;
pub use self::input::ImageInput;
pub use self::output::{OcrBlockKind, OcrOutput, OcrSpan};

/// The OCR backend contract.
///
/// Implementations send an image to an OCR service and return a
/// `Vec<OcrOutput>` — one block per page or text region, with
/// per-word spans populated and bounding boxes preserved on each
/// location.
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
/// [`ContentSource`]: nvisy_core::entity::ContentSource
#[async_trait::async_trait]
pub trait Backend: Send + Sync + 'static {
    /// Backend identity (model / service name + provenance kind).
    ///
    /// The document-side extraction phase reads this after recognition
    /// runs and stamps it into [`ImageExtraction::Ocr`] on the
    /// document's metadata, so the audit records *which* OCR pass
    /// produced the document.
    ///
    /// [`ImageExtraction::Ocr`]: nvisy_core::modality::ImageExtraction::Ocr
    fn provenance(&self) -> ModelProvenance;

    /// Run OCR on a single image under `ctx`.
    async fn run(&self, image: &ImageInput, ctx: Context<'_>) -> Result<Vec<OcrOutput>, Error>;

    /// Run OCR on each of `images` under one shared [`Context`],
    /// concatenating the per-image outputs.
    ///
    /// `images` is assumed to be slices of the same source (see
    /// the trait-level docs). The default impl dispatches
    /// concurrently via `futures::join_all` and concatenates;
    /// backends with native batching override it.
    async fn run_batch(
        &self,
        images: &[ImageInput],
        ctx: Context<'_>,
    ) -> Result<Vec<OcrOutput>, Error> {
        let pending: Vec<_> = images.iter().map(|img| self.run(img, ctx)).collect();
        let results: Vec<Result<Vec<OcrOutput>, Error>> = futures::future::join_all(pending).await;
        let mut merged: Vec<OcrOutput> = Vec::new();
        for r in results {
            merged.extend(r?);
        }
        Ok(merged)
    }
}
