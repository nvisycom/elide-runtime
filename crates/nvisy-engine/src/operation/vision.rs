//! Visual extraction: OCR text extraction, verification, and computer vision.
//!
//! Extracts text and entities from image documents by running OCR,
//! optionally verifying detected entities against the source image,
//! and optionally running computer vision entity detection.

use futures::StreamExt;
use nvisy_codec::handler::{ImageData, ImageHandler};
use nvisy_codec::{Document, Span};
use nvisy_core::math::BoundingBox;
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_http::HttpClient;
use nvisy_ocr::{ImageFormat, ImageInput, OcrEngine, RunParams};
use nvisy_ontology::entity::{
    Entities, Entity, ExtractionMethod, ImageLocation, RecognitionMethod,
};
use nvisy_rig::agent::{CvEntity, DetectionConfig, OcrAgent};

use crate::graph::RetryPolicy;
use crate::operation::envelope::DetectedEntities;
use crate::operation::{DocumentEnvelope, Operation, ParallelContext, SharedContext};
use crate::pipeline::RuntimeConfig;

const TARGET: &str = "nvisy_engine::op::visual_extraction";

/// Visual extraction operation: OCR + optional verification + optional CV.
pub struct VisualExtraction {
    ocr: OcrOp,
    verifier: Option<VerifyOp>,
    shared: SharedContext,
    retry: Option<RetryPolicy>,
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
    pub fn connect(
        cfg: &crate::graph::VisualExtraction,
        config: &RuntimeConfig,
        http_client: &HttpClient,
        shared: SharedContext,
        retry: Option<RetryPolicy>,
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

        Ok(Self {
            ocr,
            verifier,
            shared,
            retry,
        })
    }

    pub(crate) async fn process(
        &self,
        mut envelope: DocumentEnvelope,
    ) -> Result<DocumentEnvelope, Error> {
        let Document::Image(ref handler) = envelope.document else {
            return Ok(envelope);
        };

        let image_spans: Vec<_> = handler.image_spans().await.collect().await;
        let ocr_spans: Vec<Span<(), _>> = image_spans
            .into_iter()
            .map(|s| Span::new((), s.data).with_source(s.source))
            .collect();

        let retry = self.retry.as_ref();
        let ocr_ref = &self.ocr;
        let _ocr_output = RetryPolicy::call(retry, || {
            let spans = ocr_spans.clone();
            let shared = self.shared.clone();
            async move {
                let input = ParallelContext::new(spans, shared);
                ocr_ref.call(input).await
            }
        })
        .await?;

        if let Some(ref verifier) = self.verifier
            && !envelope.entities.is_empty()
        {
            let verify_spans: Vec<_> = match &envelope.document {
                Document::Image(h) => h
                    .image_spans()
                    .await
                    .collect::<Vec<_>>()
                    .await
                    .into_iter()
                    .map(|s| Span::new((), s.data).with_source(s.source))
                    .collect(),
                _ => Vec::new(),
            };

            let do_verify = || {
                let input = VerifyInput {
                    image_spans: verify_spans.clone(),
                    entities: envelope.entities.clone(),
                };
                let shared = self.shared.clone();
                async move {
                    let ctx = ParallelContext::new(input, shared);
                    verifier.call(ctx).await
                }
            };
            match RetryPolicy::call(retry, do_verify).await {
                Ok(output) => envelope.apply(output.into_inner()),
                Err(e) => tracing::warn!(
                    target: TARGET,
                    error = %e,
                    "OCR verification failed, keeping unverified entities"
                ),
            }
        }

        Ok(envelope)
    }
}

// --- Internal: OCR ---

struct OcrOp {
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
    ) -> Result<Vec<nvisy_ocr::ImageOutput>> {
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
    type Output = ParallelContext<Vec<nvisy_ocr::ImageOutput>>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|spans| self.extract(spans)).await
    }
}

// --- Internal: Verification ---

struct VerifyInput {
    image_spans: Vec<Span<(), ImageData>>,
    entities: Entities,
}

impl Clone for VerifyInput {
    fn clone(&self) -> Self {
        Self {
            image_spans: self.image_spans.clone(),
            entities: self.entities.clone(),
        }
    }
}

struct VerifyOp {
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
