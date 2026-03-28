//! Visual extraction operation.
//!
//! Runs at **phase 1**, after ingestion. Extracts text and entities from
//! image documents by running OCR, optionally verifying detected entities
//! against the source image, and optionally running computer vision.

use nvisy_codec::Span;
use nvisy_codec::handler::ImageData;
use nvisy_core::math::BoundingBox;
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::entity::{
    Entities, Entity, ExtractionMethod, ImageLocation, RecognitionMethod,
};
use nvisy_provider::agent::{CvEntity, OcrAgent};
use nvisy_provider::http::HttpClient;
use nvisy_provider::ocr::{ImageFormat, ImageInput, OcrEngine, RunParams};

use crate::graph::VisualExtraction as VisualExtractionCfg;
use crate::operation::Operation;
use crate::operation::context::ParallelContext;
use crate::operation::envelope::DetectedEntities;
use crate::pipeline::RuntimeConfig;

const TARGET: &str = "nvisy_engine::op::visual_extraction";

/// Visual extraction operation: OCR + optional verification + optional CV.
pub struct VisualExtraction {
    ocr: OcrOp,
    verifier: Option<VerifyOp>,
}

impl VisualExtraction {
    fn build_ocr_agent(config: &RuntimeConfig) -> Result<OcrAgent> {
        let llm = config.llm.as_ref().ok_or_else(|| {
            Error::new(
                ErrorKind::Validation,
                "OCR verification requires an LLM provider",
            )
        })?;
        let provider = llm.provider.as_ref().ok_or_else(|| {
            Error::new(
                ErrorKind::Validation,
                "OCR verification requires an LLM provider",
            )
        })?;
        OcrAgent::new(provider, llm.policy.clone().unwrap_or_default())
            .map_err(|e| Error::runtime(e.to_string(), "ocr-agent", false))
    }

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
        let ocr_engine = ocr_provider.into_engine_with_client(http_client.clone());
        let ocr = OcrOp::new(ocr_engine, ocr_params);

        let verifier = if cfg.verification {
            match Self::build_ocr_agent(config) {
                Ok(agent) => Some(VerifyOp::new(agent)),
                Err(e) => {
                    tracing::warn!(target: TARGET, error = %e, "OCR verification unavailable, skipping");
                    None
                }
            }
        } else {
            None
        };

        if cfg.entity_detection {
            tracing::warn!(target: TARGET, "CV entity detection not yet configurable, skipping");
        }

        Ok(Self { ocr, verifier })
    }

    /// Access the inner OCR operation for direct dispatch.
    pub(crate) fn ocr(&self) -> &OcrOp {
        &self.ocr
    }

    /// Access the optional verification operation for direct dispatch.
    pub(crate) fn verifier(&self) -> Option<&VerifyOp> {
        self.verifier.as_ref()
    }
}

pub(crate) struct OcrOp {
    engine: OcrEngine,
    params: RunParams,
}

impl OcrOp {
    fn new(engine: OcrEngine, params: RunParams) -> Self {
        Self { engine, params }
    }

    async fn extract(
        &self,
        spans: Vec<Span<(), ImageData>>,
    ) -> Result<Vec<nvisy_provider::ocr::ImageOutput>> {
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
        self.engine.run_batch(&images, &self.params).await
    }
}

impl Operation for OcrOp {
    type Input = ParallelContext<Vec<Span<(), ImageData>>>;
    type Output = ParallelContext<Vec<nvisy_provider::ocr::ImageOutput>>;

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

pub(crate) struct VerifyOp {
    agent: OcrAgent,
}

impl VerifyOp {
    fn new(agent: OcrAgent) -> Self {
        Self { agent }
    }

    async fn verify(&self, data: VerifyInput) -> Result<DetectedEntities> {
        if data.entities.is_empty() || data.image_spans.is_empty() {
            return Ok(DetectedEntities(data.entities));
        }
        let mut verified = data.entities.into_inner();
        for span in &data.image_spans {
            let image_bytes = span.data.encode_png()?;
            verified = self
                .agent
                .verify_entities(&image_bytes, verified)
                .await
                .map_err(|e| Error::runtime(e.to_string(), "ocr-verification", e.is_retryable()))?;
        }
        Ok(DetectedEntities(verified.into()))
    }
}

impl Operation for VerifyOp {
    type Input = ParallelContext<VerifyInput>;
    type Output = ParallelContext<DetectedEntities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.verify(data)).await
    }
}
