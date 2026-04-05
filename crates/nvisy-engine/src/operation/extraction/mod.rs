//! Extraction operations: visual (OCR), audial (STT), and text.
//!
//! [`ExtractionOp`] is the single entry point for the extraction phase.
//! It runs all applicable modalities based on the document type,
//! using configuration from the [`Extraction`] graph node.
//!
//! Individual modality implementations live in sub-modules:
//! - [`vision`]: OCR on images and scanned documents.
//! - [`speech`]: speech-to-text on audio.
//!
//! [`Extraction`]: nvisy_ontology::workflow::Extraction

mod speech;
mod vision;

use nvisy_codec::ContentHandle;
use nvisy_core::Result;
use nvisy_ontology::workflow::Extraction;
use nvisy_provider::http::HttpClient;

use self::speech::AudialExtractionOp;
use self::vision::VisualExtractionOp;
use crate::operation::{DocumentEnvelope, Operation};
use crate::pipeline::RuntimeConfig;

const TARGET: &str = "nvisy_engine::op::extraction";

/// Combined extraction operation for all content modalities.
///
/// Runs visual (OCR) and audial (STT) extraction based on the
/// document's content type. Both modalities are attempted — errors
/// from missing providers are silently skipped (the document may
/// not need that modality).
pub struct ExtractionOp {
    visual: Option<VisualExtractionOp>,
    audial: Option<AudialExtractionOp>,
}

impl ExtractionOp {
    /// Build from extraction config and runtime dependencies.
    ///
    /// Each modality is constructed independently. Missing providers
    /// result in that modality being `None` (skipped at runtime),
    /// not an error.
    pub fn new(cfg: &Extraction, config: &RuntimeConfig, http_client: &HttpClient) -> Self {
        let visual_cfg = cfg.visual.clone().unwrap_or_default();
        let visual = VisualExtractionOp::new(&visual_cfg, config, http_client)
            .inspect_err(|e| {
                tracing::debug!(target: TARGET, error = %e, "visual extraction unavailable");
            })
            .ok();

        let audial_cfg = cfg.audial.clone().unwrap_or_default();
        let audial = AudialExtractionOp::new(&audial_cfg, config, http_client)
            .inspect_err(|e| {
                tracing::debug!(target: TARGET, error = %e, "audial extraction unavailable");
            })
            .ok();

        Self { visual, audial }
    }
}

impl Operation for ExtractionOp {
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
            ContentHandle::Text(_) => {
                // Text documents are already structured text — no
                // extraction needed. Future: whitespace normalization.
                tracing::debug!(target: TARGET, "text document, no extraction needed");
            }
        }
        Ok(())
    }
}
