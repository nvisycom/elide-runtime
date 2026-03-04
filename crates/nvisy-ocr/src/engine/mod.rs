//! Type-erased OCR engine.

use tracing::{debug, instrument};

use nvisy_core::Error;

use crate::backend::{Backend, ImageInput, ImageOutput, RunParams};

/// Type-erased OCR engine wrapping any [`Backend`] implementation.
///
/// `Engine` owns a boxed [`Backend`] and forwards OCR requests to it,
/// providing a concrete, object-safe entry point without requiring
/// generics or `Arc<dyn Backend>` at every call site.
///
/// # Examples
///
/// ```ignore
/// use nvisy_ocr::{Engine, ImageInput, ImageFormat, RunParams};
/// use nvisy_ocr::provider::DoctrBackend;
///
/// let backend = DoctrBackend::new(client, "http://localhost:8000");
/// let engine = Engine::new(backend);
///
/// let image = ImageInput::new(png_bytes, ImageFormat::Png);
/// let output = engine.run(&image, &RunParams::default()).await?;
/// println!("{} regions detected", output.len());
/// ```
pub struct Engine {
    backend: Box<dyn Backend>,
}

impl Engine {
    /// Create a new engine from any [`Backend`] implementation.
    pub fn new(backend: impl Backend) -> Self {
        Self {
            backend: Box::new(backend),
        }
    }

    /// Run OCR on a single image.
    #[instrument(skip_all, fields(
        source = %image.source,
        image_bytes = image.len(),
        format = ?image.format,
    ))]
    pub async fn run(
        &self,
        image: &ImageInput,
        params: &RunParams,
    ) -> Result<ImageOutput, Error> {
        let output = self.backend.run(image, params).await?;
        debug!(regions = output.len(), "ocr complete");
        Ok(output)
    }

    /// Run OCR on multiple images, returning results in the same order.
    #[instrument(skip_all, fields(count = images.len()))]
    pub async fn run_batch(
        &self,
        images: &[ImageInput],
        params: &RunParams,
    ) -> Result<Vec<ImageOutput>, Error> {
        let outputs = self.backend.run_batch(images, params).await?;
        let total_regions: usize = outputs.iter().map(|o| o.len()).sum();
        debug!(total_regions, "batch ocr complete");
        Ok(outputs)
    }
}
