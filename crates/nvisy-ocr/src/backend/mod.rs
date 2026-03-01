//! OCR backend trait and configuration.

use serde::Serialize;

use nvisy_core::Error;
use nvisy_core::math::{BoundingBox, Polygon};
use nvisy_ontology::location::TextLevel;

/// Configuration passed to an [`OcrBackend`] implementation.
#[derive(Debug, Clone)]
pub struct OcrConfig {
    /// Language hint (e.g. `"eng"` for English).
    pub language: String,
    /// OCR engine to use (`"tesseract"`, `"google-vision"`, `"aws-textract"`).
    pub engine: String,
    /// Minimum confidence threshold for OCR results.
    pub confidence_threshold: f64,
}

/// A single text region returned by an OCR backend.
#[derive(Debug, Clone, Serialize)]
pub struct OcrRegion {
    /// The extracted text content.
    pub text: String,
    /// Confidence of the OCR extraction (0.0..=1.0).
    pub confidence: f64,
    /// Axis-aligned bounding box.
    pub bbox: BoundingBox,
    /// Polygon vertices for rotated text regions.
    pub polygon: Option<Polygon>,
    /// Hierarchical level of this text region.
    pub level: Option<TextLevel>,
}

/// Backend trait for OCR providers.
///
/// Implementations call an OCR engine (local or remote) and return
/// typed [`OcrRegion`] results.
#[async_trait::async_trait]
pub trait OcrBackend: Send + Sync + 'static {
    /// Run OCR on image bytes, returning detected text regions.
    async fn detect_ocr(
        &self,
        image_data: &[u8],
        mime_type: &str,
        config: &OcrConfig,
    ) -> Result<Vec<OcrRegion>, Error>;
}
