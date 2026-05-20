//! OCR backend trait and shared types.

mod input;
mod output;

use nvisy_core::Error;
pub use nvisy_core::media::ImageFormat;
use reqwest_middleware::reqwest::multipart::Part;

pub use self::input::ImageInput;
pub use self::output::ImageOutput;

/// Build a multipart [`Part`] from an [`ImageInput`].
pub(crate) fn image_part(image: &ImageInput) -> Result<Part, Error> {
    let filename = format!("image.{}", image.format.extension());
    Part::bytes(image.data.to_vec())
        .file_name(filename)
        .mime_str(image.mime_type())
        .map_err(|e| Error::runtime(format!("invalid mime type: {e}"), "ocr", false))
}

/// Parameters passed to a [`Backend`] implementation.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct RunParams {
    /// Minimum confidence threshold for OCR results (0.0..=1.0).
    pub confidence_threshold: f64,
}

impl RunParams {
    /// Create params with the given confidence threshold.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] with [`ErrorKind::Validation`] if
    /// `confidence_threshold` is not in `0.0..=1.0` (including `NaN`).
    ///
    /// [`ErrorKind::Validation`]: nvisy_core::ErrorKind::Validation
    pub fn new(confidence_threshold: f64) -> Result<Self, Error> {
        if !(0.0..=1.0).contains(&confidence_threshold) {
            return Err(Error::validation(
                format!("confidence_threshold must be in 0.0..=1.0, got {confidence_threshold}"),
                "ocr",
            ));
        }
        Ok(Self {
            confidence_threshold,
        })
    }
}

/// Backend trait for OCR providers.
///
/// Implementations send image bytes to an OCR service and return
/// hierarchical [`ImageOutput`] results with page/block/line/word structure.
///
/// Confidence values **must** be normalised to 0.0..=1.0 before
/// populating word confidence. Backends whose upstream API uses
/// a different scale (e.g. AWS Textract returns 0–100) are
/// responsible for converting.
#[async_trait::async_trait]
pub trait Backend: Send + Sync + 'static {
    /// Run OCR on a single image.
    async fn run(&self, image: &ImageInput, params: &RunParams) -> Result<ImageOutput, Error>;

    /// Run OCR on multiple images, returning results in the same order.
    ///
    /// The default implementation runs all images concurrently. Backends
    /// that need sequential processing or have native batch APIs can
    /// override.
    async fn run_batch(
        &self,
        images: &[ImageInput],
        params: &RunParams,
    ) -> Result<Vec<ImageOutput>, Error> {
        let futures: Vec<_> = images.iter().map(|img| self.run(img, params)).collect();
        let results: Vec<Result<ImageOutput, Error>> = futures::future::join_all(futures).await;
        results.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_params_rejects_above_one() {
        assert!(RunParams::new(1.01).is_err());
    }

    #[test]
    fn run_params_rejects_negative() {
        assert!(RunParams::new(-0.1).is_err());
    }

    #[test]
    fn run_params_rejects_nan() {
        assert!(RunParams::new(f64::NAN).is_err());
    }
}
