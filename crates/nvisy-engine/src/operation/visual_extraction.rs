//! Visual extraction operations: OCR, OCR verification, and computer vision.
//!
//! | Operation           | Description                                        |
//! |---------------------|----------------------------------------------------|
//! | [`Ocr`]             | Extracts text regions from images via OCR engine   |
//! | [`OcrVerification`] | Confirms/rejects entities against images via VLM   |
//! | [`ComputerVision`]  | Object detection + classification on images        |

use nvisy_codec::Span;
use nvisy_codec::handler::ImageData;
use nvisy_core::math::BoundingBox;
use nvisy_core::{Error, Result};
use nvisy_ocr::{ImageFormat, ImageInput, ImageOutput, OcrEngine, RunParams};
use nvisy_ontology::entity::{Entities, Entity, ExtractionMethod, ImageLocation, RecognitionMethod};
use nvisy_rig::agent::{CvAgent, CvEntity, DetectionConfig, OcrAgent};

use crate::operation::envelope::DetectedEntities;
use crate::operation::{Operation, ParallelContext};

// --- Ocr ---

/// OCR text-extraction operation: thin adapter around [`OcrEngine`].
///
/// [`OcrEngine`]: nvisy_ocr::OcrEngine
pub struct Ocr {
    engine: OcrEngine,
    params: RunParams,
}

impl Ocr {
    pub fn new(engine: OcrEngine, params: RunParams) -> Self {
        Self { engine, params }
    }

    fn to_image_input(span: &Span<(), ImageData>) -> Result<ImageInput> {
        let png_bytes = span.data.encode_png()?;
        Ok(ImageInput::with_source(
            span.source,
            png_bytes,
            ImageFormat::Png,
        ))
    }

    async fn extract(&self, spans: Vec<Span<(), ImageData>>) -> Result<Vec<ImageOutput>> {
        if spans.is_empty() {
            return Ok(Vec::new());
        }
        tracing::debug!(span_count = spans.len(), "extracting text via OCR");
        let images = spans
            .iter()
            .map(Self::to_image_input)
            .collect::<Result<Vec<_>>>()?;
        self.engine.run_batch(&images, &self.params).await
    }
}

impl Operation for Ocr {
    type Input = ParallelContext<Vec<Span<(), ImageData>>>;
    type Output = ParallelContext<Vec<ImageOutput>>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|spans| self.extract(spans)).await
    }
}

// --- OcrVerification ---

/// Input for the OCR verification operation.
pub struct OcrVerificationInput {
    pub image_spans: Vec<Span<(), ImageData>>,
    pub entities: Entities,
}

/// Verifies detected entities against source images via VLM,
/// correcting or rejecting false positives.
pub struct OcrVerification {
    agent: OcrAgent,
}

impl OcrVerification {
    pub fn new(agent: OcrAgent) -> Self {
        Self { agent }
    }

    async fn verify(&self, data: OcrVerificationInput) -> Result<DetectedEntities> {
        if data.entities.is_empty() {
            return Ok(DetectedEntities(Entities::new()));
        }
        if data.image_spans.is_empty() {
            return Ok(DetectedEntities(data.entities));
        }

        tracing::debug!(
            entity_count = data.entities.len(),
            image_count = data.image_spans.len(),
            "verifying entities against images",
        );

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

impl Operation for OcrVerification {
    type Input = ParallelContext<OcrVerificationInput>;
    type Output = ParallelContext<DetectedEntities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.verify(data)).await
    }
}

// --- ComputerVision ---

/// Computer-vision entity detection: object detection + classification.
pub struct ComputerVision {
    agent: CvAgent,
    config: DetectionConfig,
}

impl ComputerVision {
    pub fn from_agent(agent: CvAgent, config: DetectionConfig) -> Self {
        Self { agent, config }
    }

    async fn detect(&self, spans: Vec<Span<(), ImageData>>) -> Result<DetectedEntities> {
        tracing::debug!(span_count = spans.len(), "detecting entities via CV");
        let mut entities = Vec::new();

        for span in &spans {
            let png_bytes = span.data.encode_png()?;
            let cv_entities = self
                .agent
                .detect(&png_bytes, &self.config)
                .await
                .map_err(|e| Error::runtime(e.to_string(), "cv-agent", e.is_retryable()))?;

            let image_id = Some(span.source.as_uuid());
            for cv_entity in &cv_entities {
                let entity = map_cv_entity(cv_entity, image_id);
                entities.push(entity.with_parent(&span.source));
            }
        }

        Ok(DetectedEntities(entities.into()))
    }
}

impl Operation for ComputerVision {
    type Input = ParallelContext<Vec<Span<(), ImageData>>>;
    type Output = ParallelContext<DetectedEntities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|spans| self.detect(spans)).await
    }
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
