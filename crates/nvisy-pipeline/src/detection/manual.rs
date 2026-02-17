//! Manual annotation detection action.
//!
//! Converts user-provided inclusion [`Annotation`]s into full [`Entity`] objects.

use serde::Deserialize;

use crate::ontology::entity::{DetectionMethod, Entity};
use crate::ontology::detection::{Annotation, AnnotationKind};
use nvisy_core::error::Error;

use crate::action::Action;

/// Typed parameters for [`DetectManualAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectManualParams {}

/// Converts each inclusion [`Annotation`] into a full [`Entity`] with
/// `DetectionMethod::Manual` and confidence 1.0.
pub struct DetectManualAction;

#[async_trait::async_trait]
impl Action for DetectManualAction {
    type Params = DetectManualParams;
    type Input = Vec<Annotation>;
    type Output = Vec<Entity>;

    fn id(&self) -> &str {
        "detect-manual"
    }

    async fn connect(_params: Self::Params) -> Result<Self, Error> {
        Ok(Self)
    }

    async fn execute(
        &self,
        annotations: Self::Input,
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
            let entity_type = match &ann.entity_type {
                Some(t) => t.clone(),
                None => continue,
            };
            let value = ann.value.clone().unwrap_or_default();

            let mut entity = Entity::new(
                category,
                entity_type,
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
