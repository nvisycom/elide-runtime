//! OCR backend trait and shared types.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use bytes::Bytes;
use serde::Serialize;

use nvisy_core::Error;
use nvisy_core::math::{BoundingBox, Polygon};
use nvisy_ontology::location::TextLevel;

/// Image format passed to an [`OcrBackend`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
}

impl ImageFormat {
    /// MIME type string for this format.
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }
}

/// Image payload passed to [`OcrBackend::run`].
#[derive(Debug, Clone)]
pub struct ImageInput {
    pub data: Bytes,
    pub format: ImageFormat,
}

impl ImageInput {
    /// Create a new image input.
    pub fn new(data: impl Into<Bytes>, format: ImageFormat) -> Self {
        Self {
            data: data.into(),
            format,
        }
    }

    /// MIME type string for this image.
    pub fn mime_type(&self) -> &'static str {
        self.format.mime_type()
    }

    /// Encode the image data as standard base64.
    pub fn to_base64(&self) -> String {
        BASE64.encode(&self.data)
    }
}

/// Configuration passed to an [`OcrBackend`] implementation.
#[derive(Debug, Clone)]
pub struct OcrConfig {
    /// Minimum confidence threshold for OCR results (0.0..=1.0).
    pub confidence_threshold: f64,
}

impl OcrConfig {
    /// Create a config with the given confidence threshold.
    pub fn new(confidence_threshold: f64) -> Self {
        Self {
            confidence_threshold,
        }
    }
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.0,
        }
    }
}

/// A single text region returned by an OCR backend.
#[derive(Debug, Clone, Serialize)]
pub struct OcrRegion {
    /// The extracted text content.
    pub text: String,
    /// Confidence of the OCR extraction (0.0..=1.0).
    pub confidence: f64,
    /// Axis-aligned bounding box in pixel coordinates.
    pub bbox: BoundingBox,
    /// 4-point polygon for rotated or skewed text regions.
    pub polygon: Option<Polygon>,
    /// Hierarchical level of this text region.
    pub level: Option<TextLevel>,
}

/// Backend trait for OCR providers.
///
/// Implementations send image bytes to an OCR service and return
/// typed [`OcrRegion`] results with word-level bounding boxes.
#[async_trait::async_trait]
pub trait OcrBackend: Send + Sync + 'static {
    /// Run OCR on a single image, returning detected text regions.
    async fn run(
        &self,
        image: &ImageInput,
        config: &OcrConfig,
    ) -> Result<Vec<OcrRegion>, Error>;

    /// Run OCR on multiple images, returning results in the same order.
    ///
    /// The default implementation calls [`run`](Self::run) sequentially.
    /// Backends that support batch APIs can override for better throughput.
    async fn run_batch(
        &self,
        images: &[ImageInput],
        config: &OcrConfig,
    ) -> Result<Vec<Vec<OcrRegion>>, Error> {
        let mut results = Vec::with_capacity(images.len());
        for image in images {
            results.push(self.run(image, config).await?);
        }
        Ok(results)
    }
}
