//! Type-erased OCR engine wrapping any [`Backend`].
//!
//! [`Backend`]: crate::core::Backend

use std::fmt;
use std::sync::Arc;

use nvisy_core::Error;
use tracing::instrument;

use crate::core::{Backend, ImageInput, ImageOutput, OcrParams};

const TARGET: &str = "nvisy_ocr::engine";

/// Type-erased OCR engine wrapping any [`Backend`] implementation.
///
/// Owns an `Arc<dyn Backend>` and forwards OCR requests to it, providing
/// a concrete, object-safe entry point without generics at every call
/// site. The engine is `Clone` — cloning shares the backend.
///
/// [`Backend`]: crate::core::Backend
#[derive(Clone)]
pub struct OcrEngine {
    backend: Arc<dyn Backend>,
}

impl fmt::Debug for OcrEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OcrEngine").finish_non_exhaustive()
    }
}

impl OcrEngine {
    /// Create a new engine from any [`Backend`] implementation.
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
    pub async fn run(
        &self,
        image: &ImageInput,
        params: OcrParams<'_>,
    ) -> Result<ImageOutput, Error> {
        let output = self.backend.run(image, params).await?;
        tracing::debug!(target: TARGET, words = output.word_count(), "ocr complete");
        Ok(output)
    }

    /// Run OCR on multiple images, returning results in the same order.
    #[instrument(skip_all, fields(count = images.len()))]
    pub async fn run_batch(
        &self,
        images: &[ImageInput],
        params: OcrParams<'_>,
    ) -> Result<Vec<ImageOutput>, Error> {
        let outputs = self.backend.run_batch(images, params).await?;
        let words: usize = outputs.iter().map(|o| o.word_count()).sum();
        tracing::debug!(target: TARGET, words, "batch ocr complete");
        Ok(outputs)
    }
}
