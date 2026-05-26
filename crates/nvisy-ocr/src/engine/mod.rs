//! Type-erased OCR extractor wrapping any [`Backend`].
//!
//! [`Backend`]: crate::core::Backend

use std::fmt;
use std::sync::Arc;

use nvisy_core::Error;
use tracing::instrument;

use crate::core::{Backend, Context, ImageInput, ImageOutput};

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
    ) -> Result<ImageOutput, Error> {
        let output = self.backend.run(image, ctx).await?;
        tracing::debug!(target: TARGET, words = output.word_count(), "ocr complete");
        Ok(output)
    }

    /// Run OCR on multiple images, merging the per-image pages
    /// into one [`ImageOutput`]. See [`Backend::run_batch`] for
    /// the same-source assumption.
    #[instrument(skip_all, fields(count = images.len()))]
    pub async fn extract_batch(
        &self,
        images: &[ImageInput],
        ctx: Context<'_>,
    ) -> Result<ImageOutput, Error> {
        let output = self.backend.run_batch(images, ctx).await?;
        tracing::debug!(
            target: TARGET,
            words = output.word_count(),
            "batch ocr complete",
        );
        Ok(output)
    }
}
