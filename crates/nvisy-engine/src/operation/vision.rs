//! Visual extraction operation.
//!
//! Runs at **phase 1**, after ingestion. Extracts text and entities from
//! image documents by running OCR, optionally verifying detected entities
//! against the source image, and optionally running computer vision.

use nvisy_codec::Span;
use nvisy_codec::handler::ImageData;
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::entity::{
    Entities, Entity, ExtractionMethod, ImageLocation, RecognitionMethod,
};
use nvisy_ontology::math::BoundingBox;
use nvisy_ontology::workflow::VisualExtraction as VisualExtractionCfg;
use nvisy_provider::agent::{CvEntity, ImageFormat, ImageInput, ImageOutput, OcrAgent};
use nvisy_provider::http::HttpClient;

use crate::operation::Operation;
use crate::operation::context::ParallelContext;
use crate::operation::envelope::DetectedEntities;
use crate::pipeline::RuntimeConfig;

const TARGET: &str = "nvisy_engine::op::visual_extraction";

/// Visual extraction operation: OCR + optional verification + optional CV.
pub struct VisualExtraction {
    agent: OcrAgent,
}

impl VisualExtraction {
    #[allow(dead_code)]
    fn map_cv_entity(cv: &CvEntity, image_id: Option<uuid::Uuid>) -> Entity {
        let mut entity = Entity::new(
            cv.category,
            cv.entity_type,
            &cv.label,
            RecognitionMethod::Classification,
            cv.confidence,
        );
        entity.extraction_methods = vec![ExtractionMethod::ObjectDetection];
        let bbox = if cv.bbox.len() >= 4 {
            BoundingBox {
                x: cv.bbox[0],
                y: cv.bbox[1],
                width: cv.bbox[2],
                height: cv.bbox[3],
            }
        } else {
            BoundingBox::default()
        };
        entity.with_location(
            ImageLocation {
                bounding_box: bbox,
                image_id,
                page_number: None,
            }
            .into(),
        )
    }

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
}

pub(crate) struct OcrOp<'a> {
    agent: &'a OcrAgent,
}

impl<'a> OcrOp<'a> {
    pub fn new(agent: &'a OcrAgent) -> Self {
        Self { agent }
    }

    async fn extract(&self, spans: Vec<Span<(), ImageData>>) -> Result<Vec<ImageOutput>> {
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
}

impl Operation for OcrOp<'_> {
    type Input = ParallelContext<Vec<Span<(), ImageData>>>;
    type Output = ParallelContext<Vec<ImageOutput>>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|spans| self.extract(spans)).await
    }
}

pub(crate) struct VerifyInput {
    pub(crate) image_spans: Vec<Span<(), ImageData>>,
    pub(crate) entities: Entities,
}

impl Clone for VerifyInput {
    fn clone(&self) -> Self {
        Self {
            image_spans: self.image_spans.clone(),
            entities: self.entities.clone(),
        }
    }
}

pub(crate) struct VerifyOp<'a> {
    agent: &'a OcrAgent,
}

impl<'a> VerifyOp<'a> {
    pub fn new(agent: &'a OcrAgent) -> Self {
        Self { agent }
    }

    async fn verify(&self, data: VerifyInput) -> Result<DetectedEntities> {
        if data.entities.is_empty() || data.image_spans.is_empty() {
            return Ok(DetectedEntities(data.entities));
        }
        let mut verified = data.entities.into_inner();
        for span in &data.image_spans {
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

impl Operation for VerifyOp<'_> {
    type Input = ParallelContext<VerifyInput>;
    type Output = ParallelContext<DetectedEntities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.verify(data)).await
    }
}
