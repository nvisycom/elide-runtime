//! Type-erased OCR engine.

mod params;

use std::fmt;
use std::sync::Arc;

use nvisy_core::Error;
pub use params::EngineParams;
use tracing::instrument;

use crate::backend::{Backend, ImageInput, ImageOutput, RunParams};

/// Type-erased OCR engine wrapping any [`Backend`] implementation.
///
/// Owns an `Arc<dyn Backend>` and forwards OCR requests to it, providing
/// a concrete, object-safe entry point without generics at every call
/// site. The engine is `Clone` — cloning shares the backend.
///
/// # Examples
///
/// ```ignore
/// use nvisy_ocr::{Engine, ImageInput, ImageFormat, RunParams};
/// use nvisy_ocr::provider::{DoctrBackend, DoctrParams};
///
/// let backend = DoctrBackend::new(DoctrParams { base_url: "http://localhost:8000".into() });
/// let engine = Engine::new(backend);
///
/// let image = ImageInput::new(png_bytes, ImageFormat::Png);
/// let output = engine.run(&image, &RunParams::default()).await?;
/// println!("{} regions detected", output.len());
/// ```
#[derive(Clone)]
pub struct Engine {
    backend: Arc<dyn Backend>,
}

impl fmt::Debug for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Engine").finish_non_exhaustive()
    }
}

impl Engine {
    /// Create a new engine from any [`Backend`] implementation.
    pub fn new(backend: impl Backend) -> Self {
        Self {
            backend: Arc::new(backend),
        }
    }

    /// Run OCR on a single image.
    #[instrument(skip_all, fields(
        source = %image.source,
        image_bytes = image.len(),
        format = ?image.format,
    ))]
    pub async fn run(&self, image: &ImageInput, params: &RunParams) -> Result<ImageOutput, Error> {
        let output = self.backend.run(image, params).await?;
        tracing::debug!(regions = output.len(), "ocr complete");
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
        let regions: usize = outputs.iter().map(|o| o.len()).sum();
        tracing::debug!(regions, "batch ocr complete");
        Ok(outputs)
    }
}
