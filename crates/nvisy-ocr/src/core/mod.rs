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
pub use nvisy_core::media::ImageFormat;
use nvisy_ontology::document::Document;
use nvisy_ontology::modality::Image;

pub use self::context::Context;
pub use self::input::ImageInput;

/// The OCR backend contract.
///
/// Implementations send an image to an OCR service and return a
/// [`Document<Image>`] — blocks per page or text region, spans per
/// word, with bounding boxes preserved on each location.
///
/// Confidence values **must** be normalised to `0.0..=1.0` before
/// being placed on spans. Backends whose upstream API uses a
/// different scale are responsible for converting.
///
/// Implementors **must** provide [`run`]. The default
/// [`run_batch`] impl dispatches the inputs concurrently via
/// `futures::join_all` and concatenates the per-image blocks into
/// one [`Document<Image>`]. Backends with a native batch API
/// (such as a single network round-trip) override it to merge
/// server-side.
///
/// Batch entries share a single [`Context`] and are **assumed to
/// come from the same source** — the typical caller is a
/// multi-page document split into per-page images, so the language
/// hint applies uniformly and the per-image blocks can be merged
/// into one document without further bookkeeping. Mixed-source
/// inputs should be issued as separate batches.
///
/// Per-image page numbering is returned as-is — if the caller
/// needs them rebased onto a containing document, the caller knows
/// the per-image page offsets and is responsible for that rebase.
///
/// [`run`]: Self::run
/// [`run_batch`]: Self::run_batch
#[async_trait::async_trait]
pub trait Backend: Send + Sync + 'static {
    /// Run OCR on a single image under `ctx`.
    async fn run(&self, image: &ImageInput, ctx: Context<'_>) -> Result<Document<Image>, Error>;

    /// Run OCR on each of `images` under one shared [`Context`],
    /// merging the per-image blocks into one [`Document<Image>`].
    ///
    /// `images` is assumed to be slices of the same source (see
    /// the trait-level docs). The default impl dispatches
    /// concurrently via `futures::join_all` and concatenates;
    /// backends with native batching override it.
    async fn run_batch(
        &self,
        images: &[ImageInput],
        ctx: Context<'_>,
    ) -> Result<Document<Image>, Error> {
        let pending: Vec<_> = images.iter().map(|img| self.run(img, ctx)).collect();
        let results: Vec<Result<Document<Image>, Error>> = futures::future::join_all(pending).await;
        let mut merged: Document<Image> = Document::new(Default::default(), Vec::new());
        for r in results {
            merged.blocks.extend(r?.blocks);
        }
        Ok(merged)
    }
}
