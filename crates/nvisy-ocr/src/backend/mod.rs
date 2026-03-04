//! OCR backend trait and shared types.

mod input;
mod output;

pub use input::{ImageFormat, ImageInput};
pub use output::{ImageOutput, ImageRegion};

use nvisy_core::Error;
use reqwest_middleware::reqwest::multipart::Part;

/// Build a multipart [`Part`] from an [`ImageInput`].
pub(crate) fn image_part(image: &ImageInput) -> Result<Part, Error> {
    Part::bytes(image.data.to_vec())
        .file_name("image")
        .mime_str(image.mime_type())
        .map_err(|e| Error::runtime(format!("invalid mime type: {e}"), "ocr", false))
}

/// Parameters passed to a [`Backend`] implementation.
#[derive(Debug, Clone, Default)]
pub struct RunParams {
    /// Minimum confidence threshold for OCR results (0.0..=1.0).
    pub confidence_threshold: f64,
}

impl RunParams {
    /// Create params with the given confidence threshold.
    pub fn new(confidence_threshold: f64) -> Self {
        Self {
            confidence_threshold,
        }
    }
}

/// Backend trait for OCR providers.
///
/// Implementations send image bytes to an OCR service and return
/// typed [`ImageRegion`] results with word-level bounding boxes.
#[async_trait::async_trait]
pub trait Backend: Send + Sync + 'static {
    /// Run OCR on a single image.
    async fn run(
        &self,
        image: &ImageInput,
        params: &RunParams,
    ) -> Result<ImageOutput, Error>;

    /// Run OCR on multiple images, returning results in the same order.
    ///
    /// The default implementation calls [`run`](Self::run) sequentially.
    /// Backends that support batch APIs can override for better throughput.
    async fn run_batch(
        &self,
        images: &[ImageInput],
        params: &RunParams,
    ) -> Result<Vec<ImageOutput>, Error> {
        let mut results = Vec::with_capacity(images.len());
        for image in images {
            results.push(self.run(image, params).await?);
        }
        Ok(results)
    }
}
