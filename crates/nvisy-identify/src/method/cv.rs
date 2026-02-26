//! Computer-vision detection adapter wrapping [`CvAgent`] from `nvisy-rig`.
//!
//! Detects entities in image spans by delegating to the CvAgent's
//! object-detection + LLM-classification pipeline.

use nvisy_codec::handler::{ImageData, Span};
use nvisy_core::Error;
use nvisy_rig::{CvAgent, CvEntity, DetectionConfig};

use crate::{DetectionMethod, Entity, ImageLocation, Location};
use crate::{ParallelContext, DetectionService};
use nvisy_core::math::BoundingBox;

/// Computer-vision detection method — thin adapter around [`CvAgent`].
pub struct CvMethod {
    agent: CvAgent,
    config: DetectionConfig,
}

impl CvMethod {
    /// Create a new CV method from a pre-built agent and detection config.
    pub fn from_agent(agent: CvAgent, config: DetectionConfig) -> Self {
        Self { agent, config }
    }
}

#[async_trait::async_trait]
impl DetectionService<(), ImageData> for CvMethod {
    type Context = ParallelContext;

    async fn detect(
        &self,
        spans: Vec<Span<(), ImageData>>,
    ) -> Result<Vec<Entity>, Error> {
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

        Ok(entities)
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
    .with_location(Location::Image(ImageLocation {
        bounding_box: BoundingBox {
            x: cv.bbox[0],
            y: cv.bbox[1],
            width: cv.bbox[2],
            height: cv.bbox[3],
        },
        image_id: None,
        page_number: None,
    }))
}
