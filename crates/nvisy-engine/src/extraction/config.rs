//! [`ExtractionConfig`]: deployment-time `[extractor.*]` configuration
//! the engine builds an [`ExtractionEngine`] from at startup.
//!
//! [`ExtractionEngine`]: super::ExtractionEngine

use serde::{Deserialize, Serialize};

#[cfg(feature = "audio")]
use super::audio::SttExtractorConfig;
#[cfg(feature = "image")]
use super::image::OcrExtractorConfig;

/// Configuration for the [`ExtractionEngine`] registry.
///
/// Each field maps to a `[extractor.*]` section in `Nvisy.toml`.
/// `None` opts the technique out entirely.
///
/// [`ExtractionEngine`]: super::ExtractionEngine
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractionConfig {
    /// `[extractor.ocr]` — OCR text extraction from images.
    #[cfg(feature = "image")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr: Option<OcrExtractorConfig>,
    /// `[extractor.stt]` — speech-to-text transcription.
    #[cfg(feature = "audio")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stt: Option<SttExtractorConfig>,
}

impl ExtractionConfig {
    /// `true` when every technique is `None`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        #[cfg(feature = "image")]
        let ocr_empty = self.ocr.is_none();
        #[cfg(not(feature = "image"))]
        let ocr_empty = true;
        #[cfg(feature = "audio")]
        let stt_empty = self.stt.is_none();
        #[cfg(not(feature = "audio"))]
        let stt_empty = true;
        ocr_empty && stt_empty
    }
}
