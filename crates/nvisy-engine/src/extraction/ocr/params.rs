//! [`OcrExtractorConfig`]: full bundle for constructing an
//! [`OcrExtractor`].
//!
//! [`OcrExtractor`]: super::OcrExtractor

use nvisy_ocr::{OcrProvider, RunParams as OcrRunParams};
use serde::{Deserialize, Serialize};

/// `[extractor.ocr]` config bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrExtractorConfig {
    /// Enable this extractor. When `false`, the extractor is
    /// neither built nor dispatched, but the config is preserved
    /// so operators can toggle without losing it. Defaults to
    /// `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// OCR provider selection + connection settings.
    pub provider: OcrProvider,
    /// OCR runtime parameters (confidence thresholds, etc.).
    #[serde(default)]
    pub policy: OcrRunParams,
}

fn default_true() -> bool {
    true
}
