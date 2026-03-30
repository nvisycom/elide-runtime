//! Visual extraction operation.
//!
//! Runs at **phase 1**, after ingestion. Extracts text and entities from
//! image documents by running OCR, optionally verifying detected entities
//! against the source image, and optionally running computer vision.

use nvisy_codec::Span;
use nvisy_codec::handler::ImageData;
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::entity::Entities;
use nvisy_ontology::workflow::VisualExtraction;
use nvisy_provider::agent::{ImageFormat, ImageInput, ImageOutput, OcrAgent};
use nvisy_provider::http::HttpClient;

use crate::operation::envelope::DetectedEntities;
use crate::pipeline::RuntimeConfig;

const TARGET: &str = "nvisy_engine::op::visual_extraction";

/// Visual extraction operation: OCR + optional verification + optional CV.
pub struct VisualExtractionOp {
    agent: OcrAgent,
}

impl VisualExtractionOp {
    /// Build from graph config and runtime dependencies.
    pub fn new(
        cfg: &VisualExtraction,
        config: &RuntimeConfig,
        http_client: &HttpClient,
    ) -> Result<Self> {
        let ocr_section = config.ocr.as_ref();
        let ocr_provider = ocr_section
            .and_then(|s| s.provider.clone())
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Validation,
                    "visual_extraction requires an OCR provider",
                )
            })?;
        let ocr_params = ocr_section
            .and_then(|s| s.policy.clone())
            .unwrap_or_default();

        let mut agent = OcrAgent::new(ocr_provider, ocr_params, http_client);

        if cfg.verification {
            let llm = config.llm.as_ref();
            let llm_provider = llm.and_then(|s| s.provider.as_ref());
            let llm_config = llm.and_then(|s| s.policy.clone()).unwrap_or_default();

            match llm_provider {
                Some(provider) => {
                    agent = agent
                        .with_verification(provider, llm_config)
                        .map_err(|e| Error::runtime(e.to_string(), "ocr-agent", false))?;
                }
                None => {
                    tracing::warn!(
                        target: TARGET,
                        "OCR verification requires an LLM provider, skipping"
                    );
                }
            }
        }

        if cfg.entity_detection {
            tracing::warn!(
                target: TARGET,
                "CV entity detection not yet configurable, skipping"
            );
        }

        Ok(Self { agent })
    }

    /// Access the OCR agent for direct dispatch.
    pub(crate) fn agent(&self) -> &OcrAgent {
        &self.agent
    }

    /// Run OCR extraction on a batch of image spans.
    pub(crate) async fn extract(
        &self,
        spans: Vec<Span<(), ImageData>>,
    ) -> Result<Vec<ImageOutput>> {
        if spans.is_empty() {
            return Ok(Vec::new());
        }
        let images = spans
            .iter()
            .map(|span| {
                let png_bytes = span.data.encode_png()?;
                Ok(ImageInput::with_source(
                    span.source,
                    png_bytes,
                    ImageFormat::Png,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        self.agent.run_batch(&images).await
    }

    /// Verify detected entities against the source images.
    pub(crate) async fn verify(
        &self,
        image_spans: &[Span<(), ImageData>],
        entities: Entities,
    ) -> Result<DetectedEntities> {
        if entities.is_empty() || image_spans.is_empty() {
            return Ok(DetectedEntities(entities));
        }
        let mut verified = entities.into_inner();
        for span in image_spans {
            let png_bytes = span.data.encode_png()?;
            let image = ImageInput::with_source(span.source, png_bytes, ImageFormat::Png);
            verified = self
                .agent
                .verify_entities(&image, verified)
                .await
                .map_err(|e| Error::runtime(e.to_string(), "ocr-verification", e.is_retryable()))?;
        }
        Ok(DetectedEntities(verified.into()))
    }
}
