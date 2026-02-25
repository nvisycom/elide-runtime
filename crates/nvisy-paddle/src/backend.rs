//! OCR backend trait and configuration.

use serde_json::Value;

use nvisy_core::Error;

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

/// Backend trait for OCR providers.
///
/// Implementations call an external OCR service and return raw JSON
/// results.  Entity construction is handled by the consuming crate.
#[async_trait::async_trait]
pub trait OcrBackend: Send + Sync + 'static {
    /// Run OCR on image bytes, returning raw dicts.
    async fn detect_ocr(
        &self,
        image_data: &[u8],
        mime_type: &str,
        config: &OcrConfig,
    ) -> Result<Vec<Value>, Error>;
}
