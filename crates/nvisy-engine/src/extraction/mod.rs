//! Extraction: per-technique extractors + the [`Extractors`] registry
//! that holds them.
//!
//! Three techniques today, each built once at engine startup from a
//! `[extractor.*]` config section and shared across every run:
//!
//! - [`OcrExtractor`] — pure OCR (`[extractor.ocr]`).
//! - [`SttExtractor`] — speech-to-text (`[extractor.stt]`).
//! - [`VlmExtractor`] — vision-language model verifier
//!   (`[extractor.vlm]`).
//!
//! Dispatch is driven by content type:
//!
//! - Image / rich → OCR (if configured), then VLM (if configured).
//! - Audio → STT (if configured).
//! - Text / tabular → no extraction needed.
//!
//! The workflow [`Extraction`] node carries per-call flags
//! (verification, diarization) that customize how each extractor
//! runs. Each `Extractors::run` call honors those flags by
//! activating/deactivating individual techniques.
//!
//! [`Extraction`]: crate::extraction::Extraction

#[cfg(feature = "image")]
mod ocr;
#[cfg(feature = "audio")]
mod stt;
#[cfg(feature = "image")]
mod vlm;
mod workflow;

#[cfg(any(feature = "image", feature = "audio"))]
use std::sync::Arc;

#[cfg(any(feature = "image", feature = "rich", feature = "audio"))]
use nvisy_codec::ContentHandle;
use nvisy_core::Result;
use serde::{Deserialize, Serialize};

#[cfg(feature = "image")]
pub use self::ocr::{OcrExtractor, OcrExtractorConfig};
#[cfg(feature = "audio")]
pub use self::stt::{SttExtractor, SttExtractorConfig};
#[cfg(feature = "image")]
pub use self::vlm::{VlmExtractor, VlmExtractorConfig};
pub use self::workflow::{AudialExtraction, Extraction, TextExtraction, VisualExtraction};
use crate::envelope::DocumentEnvelope;

#[cfg(any(feature = "image", feature = "rich", feature = "audio"))]
const TARGET: &str = "nvisy_engine::extraction";

/// Registry of pre-built extractors, one per technique.
///
/// Each slot is `Option<Arc<_>>` because the corresponding
/// `[extractor.*]` section is itself optional — operators only
/// configure the techniques they need.
#[derive(Default, Clone)]
pub struct Extractors {
    /// Pre-built OCR extractor (when `[extractor.ocr]` is set).
    #[cfg(feature = "image")]
    pub ocr: Option<Arc<OcrExtractor>>,
    /// Pre-built STT extractor (when `[extractor.stt]` is set).
    #[cfg(feature = "audio")]
    pub stt: Option<Arc<SttExtractor>>,
    /// Pre-built VLM extractor (when `[extractor.vlm]` is set).
    #[cfg(feature = "image")]
    pub vlm: Option<Arc<VlmExtractor>>,
}

/// Configuration for the [`Extractors`] registry.
///
/// Each field maps to a `[extractor.*]` section in `Nvisy.toml`.
/// `None` opts the technique out entirely.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractionSection {
    /// `[extractor.ocr]` — OCR text extraction from images.
    #[cfg(feature = "image")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr: Option<OcrExtractorConfig>,
    /// `[extractor.stt]` — speech-to-text transcription.
    #[cfg(feature = "audio")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stt: Option<SttExtractorConfig>,
    /// `[extractor.vlm]` — vision-language model verifier.
    #[cfg(feature = "image")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlm: Option<VlmExtractorConfig>,
}

impl ExtractionSection {
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
        #[cfg(feature = "image")]
        let vlm_empty = self.vlm.is_none();
        #[cfg(not(feature = "image"))]
        let vlm_empty = true;
        ocr_empty && stt_empty && vlm_empty
    }
}

impl Extractors {
    /// Build the registry once from an [`ExtractionSection`].
    ///
    /// Each opted-in section drives one extractor construction.
    /// Construction is eager — HTTP clients, OCR engines, and
    /// VLM agents all build here so per-run dispatch stays cheap.
    ///
    /// # Errors
    ///
    /// Returns the first construction error encountered (HTTP
    /// client setup, VLM agent build, STT service build).
    pub fn from_config(cfg: &ExtractionSection) -> Result<Self> {
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
        #[cfg(feature = "image")]
        let vlm = cfg
            .vlm
            .as_ref()
            .filter(|c| c.enabled)
            .map(|c| VlmExtractor::from_config(c.clone()).map(Arc::new))
            .transpose()?;
        Ok(Self {
            #[cfg(feature = "image")]
            ocr,
            #[cfg(feature = "audio")]
            stt,
            #[cfg(feature = "image")]
            vlm,
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
        #[cfg(feature = "image")]
        let vlm_empty = self.vlm.is_none();
        #[cfg(not(feature = "image"))]
        let vlm_empty = true;
        ocr_empty && stt_empty && vlm_empty
    }

    /// Dispatch extraction for the envelope's content type.
    ///
    /// - Image / rich → OCR (when configured), then VLM verifier
    ///   (when configured and `extraction.visual.verification` is
    ///   set).
    /// - Audio → STT (when configured).
    /// - Text / tabular → no-op (already structured).
    ///
    /// Techniques without a configured extractor are skipped
    /// silently — the document simply may not need that technique.
    pub async fn run(
        &self,
        envelope: &mut DocumentEnvelope,
        extraction: &Extraction,
    ) -> Result<()> {
        #[cfg(not(any(feature = "image", feature = "rich", feature = "audio")))]
        let _ = (envelope, extraction);
        #[cfg(any(feature = "image", feature = "rich", feature = "audio"))]
        match &envelope.document.handle {
            #[cfg(any(feature = "image", feature = "rich"))]
            handle if matches_visual(handle) => {
                #[cfg(feature = "image")]
                if let Some(ref ocr) = self.ocr {
                    tracing::debug!(target: TARGET, "running OCR extraction");
                    ocr.run(envelope).await?;
                }
                #[cfg(feature = "image")]
                {
                    let run_vlm = extraction.visual.as_ref().is_some_and(|v| v.verification);
                    if run_vlm && let Some(ref vlm) = self.vlm {
                        tracing::debug!(target: TARGET, "running VLM verification");
                        vlm.run(envelope).await?;
                    }
                }
            }
            #[cfg(feature = "audio")]
            ContentHandle::Audio(_) => {
                if let Some(ref stt) = self.stt {
                    let diarization = extraction.audial.as_ref().is_some_and(|a| a.diarization);
                    tracing::debug!(target: TARGET, "running STT extraction");
                    stt.run(envelope, diarization).await?;
                }
            }
            _ => {
                tracing::debug!(target: TARGET, "structured document, no extraction needed");
            }
        }
        Ok(())
    }
}

#[cfg(any(feature = "image", feature = "rich"))]
fn matches_visual(handle: &ContentHandle) -> bool {
    match handle {
        #[cfg(feature = "image")]
        ContentHandle::Image(_) => true,
        #[cfg(feature = "rich")]
        ContentHandle::Rich(_) => true,
        _ => false,
    }
}
