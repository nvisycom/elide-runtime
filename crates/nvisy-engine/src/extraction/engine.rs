//! [`ExtractionEngine`] + [`ExtractionConfig`]: the per-run registry
//! of pre-built extractors and its deployment-time configuration.
//!
//! One slot per technique (`ocr`, `stt`), each `Option<Arc<_>>`
//! because the corresponding `[extractor.*]` section is itself
//! optional — operators only configure the techniques they need.
//! Construction is eager: HTTP clients and OCR/STT engines build
//! once at startup so per-run dispatch stays cheap.

use std::sync::Arc;

use nvisy_core::Result;
use serde::{Deserialize, Serialize};

#[cfg(feature = "audio")]
use super::audio::{SttExtractor, SttExtractorConfig};
#[cfg(feature = "image")]
use super::image::{OcrExtractor, OcrExtractorConfig};

/// Registry of pre-built extractors, one per technique.
///
/// Each slot is `Option<Arc<_>>` because the corresponding
/// `[extractor.*]` section is itself optional — operators only
/// configure the techniques they need.
#[derive(Default, Clone)]
pub struct ExtractionEngine {
    /// Pre-built OCR extractor (when `[extractor.ocr]` is set).
    #[cfg(feature = "image")]
    pub ocr: Option<Arc<OcrExtractor>>,
    /// Pre-built STT extractor (when `[extractor.stt]` is set).
    #[cfg(feature = "audio")]
    pub stt: Option<Arc<SttExtractor>>,
}

/// Configuration for the [`ExtractionEngine`] registry.
///
/// Each field maps to a `[extractor.*]` section in `Nvisy.toml`.
/// `None` opts the technique out entirely.
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

impl ExtractionEngine {
    /// Build the registry once from an [`ExtractionConfig`].
    ///
    /// Each opted-in section drives one extractor construction.
    /// Construction is eager — HTTP clients and OCR/STT engines
    /// build here so per-run dispatch stays cheap.
    ///
    /// # Errors
    ///
    /// Returns the first construction error encountered.
    pub fn from_config(cfg: &ExtractionConfig) -> Result<Self> {
        #[cfg(not(any(feature = "image", feature = "audio")))]
        let _ = cfg;
        #[cfg(feature = "image")]
        let ocr = cfg
            .ocr
            .as_ref()
            .filter(|c| c.enabled)
            .map(|c| OcrExtractor::from_config(c.clone()).map(Arc::new))
            .transpose()?;
        #[cfg(feature = "audio")]
        let stt = cfg
            .stt
            .as_ref()
            .filter(|c| c.enabled)
            .map(|c| SttExtractor::from_config(c.clone()).map(Arc::new))
            .transpose()?;
        Ok(Self {
            #[cfg(feature = "image")]
            ocr,
            #[cfg(feature = "audio")]
            stt,
        })
    }

    /// `true` when no extractors are configured.
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
