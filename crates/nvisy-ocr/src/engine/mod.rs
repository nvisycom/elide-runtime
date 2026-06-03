//! Type-erased OCR extractor wrapping any [`Backend`].
//!
//! [`Backend`]: crate::core::Backend

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use nvisy_core::entity::ModelProvenance;
use nvisy_core::modality::{Image, ImageExtraction};
use nvisy_core::{Error, Extractor as CoreExtractor, RecognizerInput, Result};
use tracing::instrument;

use crate::core::{Backend, Context, ImageFormat, ImageInput, OcrOutput};

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

    /// Provenance of the wrapped backend, forwarded from
    /// [`Backend::provenance`].
    pub fn provenance(&self) -> ModelProvenance {
        self.backend.provenance()
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
    ) -> Result<Vec<OcrOutput>, Error> {
        let blocks = self.backend.run(image, ctx).await?;
        tracing::debug!(
            target: TARGET,
            spans = blocks.iter().map(|b| b.spans.len()).sum::<usize>(),
            "ocr complete",
        );
        Ok(blocks)
    }

    /// Run OCR on multiple images, concatenating the per-image
    /// blocks. See [`Backend::run_batch`] for the same-source
    /// assumption.
    #[instrument(skip_all, fields(count = images.len()))]
    pub async fn extract_batch(
        &self,
        images: &[ImageInput],
        ctx: Context<'_>,
    ) -> Result<Vec<OcrOutput>, Error> {
        let blocks = self.backend.run_batch(images, ctx).await?;
        tracing::debug!(
            target: TARGET,
            spans = blocks.iter().map(|b| b.spans.len()).sum::<usize>(),
            "batch ocr complete",
        );
        Ok(blocks)
    }
}

/// Bridge `nvisy_ocr::Extractor` into the toolkit-side
/// [`nvisy_core::Extractor<Image>`] surface. The extractor's output
/// is the backend-shaped `Vec<OcrOutput>`; consumers translate that
/// into per-document `Block<Image>` values.
///
/// The bridge assumes the input bytes are PNG-encoded. Callers that
/// hold images in other formats should re-encode before constructing
/// the [`nvisy_core::ImageData`] payload.
#[async_trait]
impl CoreExtractor<Image> for Extractor {
    type Output = Vec<OcrOutput>;

    fn extraction(&self) -> ImageExtraction {
        ImageExtraction::Ocr(self.provenance())
    }

    async fn extract(&self, input: &RecognizerInput<Image>) -> Result<Self::Output> {
        let image_input = ImageInput::new(input.data.bytes.clone(), ImageFormat::Png);
        let mut ocr_ctx = Context::default();
        if let Some(ref lang) = input.language {
            ocr_ctx = ocr_ctx.with_language(lang);
        }
        if let Some(corr_id) = input.correlation_id {
            ocr_ctx = ocr_ctx.with_correlation_id(corr_id);
        }
        self.extract_inner(&image_input, ocr_ctx).await
    }
}

impl Extractor {
    /// Internal helper so the trait impl can re-use the same body as
    /// the inherent [`extract`] method without recursing through the
    /// trait dispatch.
    ///
    /// [`extract`]: Self::extract
    async fn extract_inner(
        &self,
        image: &ImageInput,
        ctx: Context<'_>,
    ) -> Result<Vec<OcrOutput>, Error> {
        self.backend.run(image, ctx).await
    }
}
