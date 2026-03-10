//! OCR backend trait and shared types.

mod input;
mod output;

pub use input::ImageInput;
use nvisy_core::Error;
pub use nvisy_core::fs::ImageFormat;
pub use output::{Block, BlockKind, ImageOutput, Line, Page, Word};
use reqwest_middleware::reqwest::Response;
use reqwest_middleware::reqwest::multipart::Part;

/// Build a multipart [`Part`] from an [`ImageInput`].
pub(crate) fn image_part(image: &ImageInput) -> Result<Part, Error> {
    let filename = format!("image.{}", image.format.extension());
    Part::bytes(image.data.to_vec())
        .file_name(filename)
        .mime_str(image.mime_type())
        .map_err(|e| Error::runtime(format!("invalid mime type: {e}"), "ocr", false))
}

/// Check an HTTP response status code, returning an error for non-success.
pub(crate) async fn check_response(resp: Response, provider: &str) -> Result<Response, Error> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }

    let body = resp.text().await.unwrap_or_default();
    Err(Error::connection(
        format!("{provider} returned {status}: {body}"),
        format!("{provider}_ocr"),
        status.is_server_error(),
    ))
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
    /// # Panics
    ///
    /// Panics if `confidence_threshold` is not in `0.0..=1.0`.
    pub fn new(confidence_threshold: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&confidence_threshold),
            "confidence_threshold must be in 0.0..=1.0, got {confidence_threshold}"
        );
        Self {
            confidence_threshold,
        }
    }
}

/// Backend trait for OCR providers.
///
/// Implementations send image bytes to an OCR service and return
/// hierarchical [`ImageOutput`] results with page/block/line/word structure.
///
/// Confidence values **must** be normalised to 0.0..=1.0 before
/// populating [`Word::confidence`]. Backends whose upstream API uses
/// a different scale (e.g. AWS Textract returns 0–100) are
/// responsible for converting.
#[async_trait::async_trait]
pub trait Backend: Send + Sync + 'static {
    /// Run OCR on a single image.
    async fn run(&self, image: &ImageInput, params: &RunParams) -> Result<ImageOutput, Error>;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "confidence_threshold must be in 0.0..=1.0")]
    fn run_params_rejects_above_one() {
        RunParams::new(1.01);
    }

    #[test]
    #[should_panic(expected = "confidence_threshold must be in 0.0..=1.0")]
    fn run_params_rejects_negative() {
        RunParams::new(-0.1);
    }

    #[test]
    #[should_panic(expected = "confidence_threshold must be in 0.0..=1.0")]
    fn run_params_rejects_nan() {
        RunParams::new(f64::NAN);
    }
}
