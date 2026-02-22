//! Manual annotation detection action.
//!
//! Converts user-provided inclusion [`Annotation`]s into full [`Entity`] objects.

use serde::Deserialize;

use crate::{DetectionMethod, Entity, Annotation, AnnotationKind};
use nvisy_core::Error;

/// Typed parameters for [`DetectManualAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectManualParams {}

/// Converts each inclusion [`Annotation`] into a full [`Entity`] with
/// `DetectionMethod::Manual` and confidence 1.0.
pub struct DetectManualAction;

impl DetectManualAction {
    pub async fn connect(_params: DetectManualParams) -> Result<Self, Error> {
        Ok(Self)
    }

    pub async fn execute(
        &self,
        annotations: Vec<Annotation>,
    ) -> Result<Vec<Entity>, Error> {
        let mut entities = Vec::new();

        for ann in &annotations {
            if ann.kind != AnnotationKind::Inclusion {
                continue;
            }
            let category = match &ann.category {
                Some(c) => c.clone(),
                None => continue,
            };
            let entity_kind = match ann.entity_kind {
                Some(ek) => ek,
                None => continue,
            };
            let value = ann.value.clone().unwrap_or_default();

            let mut entity = Entity::new(
                category,
                entity_kind,
                value,
                DetectionMethod::Manual,
                1.0,
            );
            entity.text_location = ann.text_location.clone();
            entity.image_location = ann.image_location.clone();
            entity.tabular_location = ann.tabular_location.clone();
            entity.audio_location = ann.audio_location.clone();
            entity.video_location = ann.video_location.clone();

            entities.push(entity);
        }

        Ok(entities)
    }
}
