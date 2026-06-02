//! Extraction config: deployment-time `[extractor.*]` configuration
//! the engine builds an
//! [`ExtractionEngine`] from at
//! startup, plus the per-request [`Extraction`] plan node.
//!
//! [`ExtractionEngine`]: crate::phases::extraction::ExtractionEngine

#[cfg(feature = "image")]
mod ocr;
mod plan;
#[cfg(feature = "audio")]
mod stt;

use serde::{Deserialize, Serialize};

#[cfg(feature = "image")]
pub use self::ocr::OcrExtractorConfig;
pub use self::plan::{AudioPlan, Extraction, ImagePlan, TabularPlan, TextPlan};
#[cfg(feature = "audio")]
pub use self::stt::SttExtractorConfig;

/// Configuration for the
/// [`ExtractionEngine`]
/// registry.
///
/// Each field maps to a `[extractor.*]` section in `Nvisy.toml`.
/// `None` opts the technique out entirely.
///
/// [`ExtractionEngine`]: crate::phases::extraction::ExtractionEngine
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
