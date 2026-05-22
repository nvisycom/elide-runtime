//! Extraction operations: visual (OCR), audial (STT), and text.
//!
//! [`Extraction`] is the single entry point for the extraction phase.
//! It runs all applicable modalities based on the document type,
//! using configuration from the [`Extraction`] graph node.
//!
//! Individual modality implementations live in sub-modules:
//! - [`vision`]: OCR on images and scanned documents.
//! - [`speech`]: speech-to-text on audio.
//!
//! [`Extraction`]: crate::workflow::Extraction

mod speech;
mod vision;

use nvisy_codec::ContentHandle;
use nvisy_core::Result;

use self::speech::AudialExtraction;
use self::vision::VisualExtraction;
use crate::operation::{DocumentEnvelope, Operation};
use crate::pipeline::RuntimeConfig;
use crate::workflow::Extraction as ExtractionConfig;

const TARGET: &str = "nvisy_engine::op::extraction";

/// Combined extraction operation for all content modalities.
///
/// Runs visual (OCR) and audial (STT) extraction based on the
/// document's content type. Both modalities are attempted — errors
/// from missing providers are silently skipped (the document may
/// not need that modality).
pub struct Extraction {
    visual: Option<VisualExtraction>,
    audial: Option<AudialExtraction>,
}

impl Extraction {
    /// Build from extraction config and runtime dependencies.
    ///
    /// Each modality is constructed independently and provisions
    /// its own HTTP client(s) from `RuntimeConfig`. Missing
    /// providers result in that modality being `None` (skipped at
    /// runtime), not an error.
    pub fn new(cfg: &ExtractionConfig, config: &RuntimeConfig) -> Self {
        let visual_cfg = cfg.visual.clone().unwrap_or_default();
        let visual = VisualExtraction::new(&visual_cfg, config)
            .inspect_err(|e| {
                tracing::debug!(target: TARGET, error = %e, "visual extraction unavailable");
            })
            .ok();

        let audial_cfg = cfg.audial.clone().unwrap_or_default();
        let audial = AudialExtraction::new(&audial_cfg, config)
            .inspect_err(|e| {
                tracing::debug!(target: TARGET, error = %e, "audial extraction unavailable");
            })
            .ok();

        Self { visual, audial }
    }
}

impl Operation for Extraction {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        match &envelope.document.handle {
            ContentHandle::Image(_) | ContentHandle::Rich(_) => {
                if let Some(ref op) = self.visual {
                    tracing::debug!(target: TARGET, "running visual extraction");
                    op.execute(envelope).await?;
                }
            }
            ContentHandle::Audio(_) => {
                if let Some(ref op) = self.audial {
                    tracing::debug!(target: TARGET, "running audial extraction");
                    op.execute(envelope).await?;
                }
            }
            ContentHandle::Text(_) | ContentHandle::Tabular(_) => {
                // Text and tabular documents are already structured —
                // no extraction needed. Future: whitespace normalization.
                tracing::debug!(target: TARGET, "structured document, no extraction needed");
            }
        }
        Ok(())
    }
}
