//! Per-deployment `[extractor.*]` configuration plus the per-request
//! [`Extraction`] plan.
//!
//! [`ExtractionConfig::build`] turns each opted-in `[extractor.*]`
//! section into a concrete extractor and inserts it into the toolkit-
//! side [`ExtractorRegistry`]. Per-technique sub-configs
//! ([`OcrExtractorConfig`], [`SttExtractorConfig`]) live in their own
//! files; the build path is centralised here so the two arms of each
//! backend selector enum don't need to unify their concrete types via
//! `dyn` erasure.
//!
//! [`ExtractorRegistry`]: nvisy_toolkit::extraction::ExtractorRegistry
//! [`Extraction`]: self::plan::Extraction

#[cfg(feature = "image")]
mod ocr;
mod plan;
#[cfg(feature = "audio")]
mod stt;

#[cfg(all(feature = "image", not(feature = "bento")))]
use nvisy_core::Error;
use nvisy_core::Result;
use nvisy_toolkit::extraction::ExtractorRegistry;
use serde::{Deserialize, Serialize};

#[cfg(feature = "image")]
pub use self::ocr::{OcrBackend, OcrExtractorConfig};
pub use self::plan::{AudioPlan, Extraction, ImagePlan, TabularPlan, TextPlan};
#[cfg(feature = "audio")]
pub use self::stt::SttExtractorConfig;

/// Deployment-time configuration for the extractor registry.
///
/// Each field maps to a `[extractor.*]` section in `Nvisy.toml`. A
/// `None` opts the technique out entirely; an opted-in section is
/// built once at engine startup and inserted into the toolkit-side
/// [`ExtractorRegistry`].
///
/// [`ExtractorRegistry`]: nvisy_toolkit::extraction::ExtractorRegistry
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
    /// Build the [`ExtractorRegistry`] from each opted-in section,
    /// inserting the concrete extractor directly so no `dyn` erasure
    /// is required outside the registry's internal storage.
    ///
    /// # Errors
    ///
    /// Returns the first construction error encountered.
    pub fn build(&self) -> Result<ExtractorRegistry> {
        #[allow(unused_mut)]
        let mut reg = ExtractorRegistry::new();

        #[cfg(feature = "image")]
        if let Some(ocr_cfg) = self.ocr.as_ref().filter(|c| c.enabled) {
            #[cfg(feature = "bento")]
            use nvisy_ocr::{BentoBackend, BentoParams};
            use nvisy_ocr::{Extractor as OcrEngine, NoopBackend};
            reg = match &ocr_cfg.backend {
                OcrBackend::Noop => reg.with_image_extractor(OcrEngine::new(NoopBackend)),

                #[cfg(feature = "bento")]
                OcrBackend::Bento { base_url } => {
                    let backend = BentoBackend::new(BentoParams::new(base_url.clone()))?;
                    reg.with_image_extractor(OcrEngine::new(backend))
                }

                #[cfg(not(feature = "bento"))]
                OcrBackend::Bento { .. } => {
                    return Err(Error::validation(
                        "OcrBackend::Bento requires the `bento` feature",
                        "ocr",
                    ));
                }
            };
        }

        #[cfg(feature = "audio")]
        if let Some(stt_cfg) = self.stt.as_ref().filter(|c| c.enabled) {
            use nvisy_agent::audio::stt::SttService;
            let service = SttService::new(&stt_cfg.provider, stt_cfg.agent.clone())?;
            reg = reg.with_audio_extractor(service);
        }

        Ok(reg)
    }

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
