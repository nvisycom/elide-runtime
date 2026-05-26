//! Type-erased OCR extractor wrapping any [`Backend`].
//!
//! [`Backend`]: crate::core::Backend

use std::fmt;
use std::sync::Arc;

use nvisy_core::Error;
use nvisy_ontology::document::Document;
use nvisy_ontology::modality::Image;
use tracing::instrument;

use crate::core::{Backend, Context, ImageInput};

const TARGET: &str = "nvisy_ocr::engine";

/// Type-erased OCR extractor wrapping any [`Backend`] implementation.
///
/// Owns an `Arc<dyn Backend>` and forwards OCR requests to it,
/// providing a concrete, object-safe entry point without generics
/// at every call site. The extractor is `Clone` — cloning shares
/// the backend.
///
/// [`Backend`]: crate::core::Backend
#[derive(Clone)]
pub struct Extractor {
    backend: Arc<dyn Backend>,
}

impl fmt::Debug for Extractor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Extractor").finish_non_exhaustive()
    }
}

impl Extractor {
    /// Create a new extractor from any [`Backend`] implementation.
    ///
    /// [`Backend`]: crate::core::Backend
    pub fn new(backend: impl Backend) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }

    /// Run OCR on a single image.
    #[instrument(skip_all, fields(
        image_bytes = image.len(),
        format = ?image.format,
    ))]
    pub async fn extract(
        &self,
        image: &ImageInput,
        ctx: Context<'_>,
    ) -> Result<Document<Image>, Error> {
        let output = self.backend.run(image, ctx).await?;
        tracing::debug!(target: TARGET, spans = output.spans().count(), "ocr complete");
        Ok(output)
    }

    /// Run OCR on multiple images, merging the per-image blocks
    /// into one [`Document<Image>`]. See [`Backend::run_batch`] for
    /// the same-source assumption.
    #[instrument(skip_all, fields(count = images.len()))]
    pub async fn extract_batch(
        &self,
        images: &[ImageInput],
        ctx: Context<'_>,
    ) -> Result<Document<Image>, Error> {
        let output = self.backend.run_batch(images, ctx).await?;
        tracing::debug!(
            target: TARGET,
            spans = output.spans().count(),
            "batch ocr complete",
        );
        Ok(output)
    }
}
