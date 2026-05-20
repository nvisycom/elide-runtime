//! Visual extraction operation.
//!
//! Extracts text and entities from image documents by running OCR,
//! optionally verifying detected entities against the source image,
//! and optionally running computer vision.

use nvisy_codec::Span;
use nvisy_codec::handler::ImageData;
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::entity::{Entities, ImageLocation};
use nvisy_ontology::workflow::VisualExtraction as VisualExtractionCfg;
use nvisy_provider::agent::{
    ImageFormat, ImageInput, ImageOutput, OcrAgent, VerificationCandidate,
};
use nvisy_provider::http::HttpClient;

use crate::operation::{DocumentEnvelope, Operation};
use crate::pipeline::RuntimeConfig;

const TARGET: &str = "nvisy_engine::op::extraction::visual";

/// Visual extraction operation: OCR + optional verification + optional CV.
pub(super) struct VisualExtraction {
    agent: OcrAgent,
}

impl VisualExtraction {
    /// Build from graph config and runtime dependencies.
    pub fn new(
        cfg: &VisualExtractionCfg,
        config: &RuntimeConfig,
        http_client: &HttpClient,
    ) -> Result<Self> {
        let ocr_section = config.ocr.as_ref();
        let ocr_provider = ocr_section
            .and_then(|s| s.provider.clone())
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Validation,
                    "visual extraction requires an OCR provider",
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

    /// Run OCR extraction on a batch of image spans.
    async fn extract(&self, spans: &[Span<ImageLocation, ImageData>]) -> Result<Vec<ImageOutput>> {
        if spans.is_empty() {
            return Ok(Vec::new());
        }
        let inputs = spans
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
        self.agent.run_batch(&inputs).await
    }

    /// Verify detected entities against the source images.
    async fn verify(
        &self,
        spans: &[Span<ImageLocation, ImageData>],
        entities: Entities,
        document: &crate::operation::Document,
    ) -> Result<Entities> {
        if entities.is_empty() || spans.is_empty() {
            return Ok(entities);
        }

        let mut verified = entities.into_inner();
        for span in spans {
            let png_bytes = span.data.encode_png()?;
            let image = ImageInput::with_source(span.source, png_bytes, ImageFormat::Png);
            let mut candidates = Vec::with_capacity(verified.len());
            for entity in verified {
                let value = document
                    .value_at(&entity.location)
                    .await
                    .unwrap_or_default();
                candidates.push(VerificationCandidate { entity, value });
            }
            verified = self
                .agent
                .verify_entities(&image, candidates)
                .await
                .map_err(|e| Error::runtime(e.to_string(), "ocr-verification", e.is_retryable()))?;
        }
        Ok(verified.into())
    }

    /// Collect all image spans from the document's image locations.
    async fn collect_spans(
        document: &crate::operation::Document,
    ) -> Vec<Span<ImageLocation, ImageData>> {
        let locations = document.collect_image_locations().await;
        let mut spans = Vec::with_capacity(locations.len());
        for located in locations {
            if let Some(data) = document.read_image(&located.location).await {
                spans.push(Span::from_located(located, data));
            }
        }
        spans
    }
}

impl Operation for VisualExtraction {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        let spans = Self::collect_spans(&envelope.document).await;
        if spans.is_empty() {
            return Ok(());
        }

        tracing::debug!(
            target: TARGET,
            spans = spans.len(),
            "running OCR extraction",
        );

        let ocr_output = self.extract(&spans).await?;

        // Store OCR results in image artifacts.
        if let Some(image_artifacts) = envelope.document.artifacts.as_image_mut() {
            for output in &ocr_output {
                image_artifacts.ocr_pages.extend(output.pages.clone());
            }
        }

        if self.agent.has_verifier() && !envelope.audit.entities.is_empty() {
            let verify_spans = Self::collect_spans(&envelope.document).await;
            match self
                .verify(
                    &verify_spans,
                    envelope.audit.entities.clone(),
                    &envelope.document,
                )
                .await
            {
                Ok(verified) => envelope.audit.entities = verified,
                Err(e) => tracing::warn!(
                    target: TARGET, error = %e,
                    "OCR verification failed, keeping unverified entities"
                ),
            }
        }

        Ok(())
    }
}
