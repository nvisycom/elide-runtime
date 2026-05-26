//! [`OcrExtractorConfig`]: full bundle for constructing an
//! [`OcrExtractor`].
//!
//! [`OcrExtractor`]: super::OcrExtractor

use nvisy_ocr::OcrBackend;
use serde::{Deserialize, Serialize};

/// `[extractor.ocr]` config bundle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OcrExtractorConfig {
    /// Enable this extractor. When `false`, the extractor is
    /// neither built nor dispatched, but the config is preserved
    /// so operators can toggle without losing it. Defaults to
    /// `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// OCR backend selection + connection settings.
    #[serde(default)]
    pub backend: OcrBackend,
}

fn default_true() -> bool {
    true
}
