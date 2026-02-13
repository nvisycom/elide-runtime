//! Manual annotation detection action.
//!
//! Converts user-provided inclusion [`Annotation`]s into full [`Entity`] objects.

use serde::Deserialize;

use nvisy_ontology::entity::{DetectionMethod, Entity};
use nvisy_ontology::detection::{Annotation, AnnotationKind};
use nvisy_core::error::Error;

use crate::action::Action;

/// Typed parameters for [`DetectManualAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectManualParams {}

/// Converts each inclusion [`Annotation`] into a full [`Entity`] with
/// `DetectionMethod::Manual` and confidence 1.0.
pub struct DetectManualAction {
    params: DetectManualParams,
}

#[async_trait::async_trait]
impl Action for DetectManualAction {
    type Params = DetectManualParams;
    type Input = Vec<Annotation>;
    type Output = Vec<Entity>;

    fn id(&self) -> &str {
        "detect-manual"
    }

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        Ok(Self { params })
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
            let location = match &ann.location {
                Some(l) => l.clone(),
                None => continue,
            };

            let entity = Entity::new(
                category,
                entity_type,
                value,
                DetectionMethod::Manual,
                1.0,
                location,
            );

            entities.push(entity);
        }

        Ok(entities)
    }
}
