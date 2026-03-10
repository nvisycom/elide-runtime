//! Computer-vision detection adapter wrapping [`CvAgent`] from `nvisy-rig`.
//!
//! Detects entities in image spans by delegating to the CvAgent's
//! object-detection + LLM-classification pipeline.

use nvisy_codec::Span;
use nvisy_codec::handler::ImageData;
use nvisy_core::math::BoundingBox;
use nvisy_core::{Error, Result};
use nvisy_ontology::entity::{DetectionMethod, Entities, Entity, ImageLocation};
use nvisy_rig::agent::{CvAgent, CvEntity, DetectionConfig};

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

    async fn detect(&self, spans: Vec<Span<(), ImageData>>) -> Result<Entities> {
        tracing::debug!(target: TARGET, span_count = spans.len(), "detecting entities");
        let mut entities = Vec::new();

        for span in &spans {
            let png_bytes = span.data.encode_png()?;

            let cv_entities = self
                .agent
                .detect(&png_bytes, &self.config)
                .await
                .map_err(|e| Error::runtime(e.to_string(), "cv-agent", e.is_retryable()))?;

            for cv_entity in &cv_entities {
                let entity = map_cv_entity(cv_entity);
                entities.push(entity.with_parent(&span.source));
            }
        }

        Ok(entities.into())
    }
}

impl Operation for ComputerVision {
    type Input = ParallelContext<Vec<Span<(), ImageData>>>;
    type Output = ParallelContext<Entities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|spans| self.detect(spans)).await
    }
}

/// Convert a [`CvEntity`] to an [`Entity`] with [`ImageLocation`].
fn map_cv_entity(cv: &CvEntity) -> Entity {
    Entity::new(
        cv.category.clone(),
        cv.entity_type,
        &cv.label,
        DetectionMethod::ObjectDetection,
        cv.confidence,
    )
    .with_location(
        ImageLocation {
            bounding_box: BoundingBox {
                x: cv.bbox[0],
                y: cv.bbox[1],
                width: cv.bbox[2],
                height: cv.bbox[3],
            },
            image_id: None,
            page_number: None,
        }
        .into(),
    )
}
