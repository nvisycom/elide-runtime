//! Computer-vision detection adapter wrapping [`CvAgent`] from `nvisy-rig`.
//!
//! Detects entities in image spans by delegating to the CvAgent's
//! object-detection + LLM-classification pipeline.

use nvisy_codec::Span;
use nvisy_codec::handler::ImageData;
use nvisy_core::math::BoundingBox;
use nvisy_core::{Error, Result};
use nvisy_ontology::entity::{Entity, ExtractionMethod, ImageLocation, RecognitionMethod};
use nvisy_rig::agent::{CvAgent, CvEntity, DetectionConfig};

use crate::operation::envelope::DetectedEntities;
use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::computer_vision";

/// Computer-vision detection operation: thin adapter around [`CvAgent`].
pub struct ComputerVision {
    agent: CvAgent,
    config: DetectionConfig,
}

impl ComputerVision {
    /// Create a new CV operation from a pre-built agent and detection config.
    pub fn from_agent(agent: CvAgent, config: DetectionConfig) -> Self {
        Self { agent, config }
    }

    async fn detect(&self, spans: Vec<Span<(), ImageData>>) -> Result<DetectedEntities> {
        tracing::debug!(target: TARGET, span_count = spans.len(), "detecting entities");
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

/// Convert a [`CvEntity`] to an [`Entity`] with [`ImageLocation`].
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
