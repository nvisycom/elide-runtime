//! Manual annotation detection action.
//!
//! Converts user-provided [`ManualAnnotation`]s into full [`Entity`] objects.

use serde::Deserialize;

use nvisy_ontology::ontology::entity::{DetectionMethod, Entity, EntityLocation};
use nvisy_ontology::redaction::ManualAnnotation;
use nvisy_core::error::Error;

use crate::action::Action;

/// Typed parameters for [`DetectManualAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectManualParams {}

/// Converts each [`ManualAnnotation`] into a full [`Entity`] with
/// `DetectionMethod::Manual` and confidence 1.0.
pub struct DetectManualAction {
    params: DetectManualParams,
}

#[async_trait::async_trait]
impl Action for DetectManualAction {
    type Params = DetectManualParams;
    type Input = Vec<ManualAnnotation>;
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
            let entity = Entity::new(
                ann.category,
                &ann.entity_type,
                &ann.value,
                DetectionMethod::Manual,
                1.0,
                EntityLocation {
                    start_offset: ann.start_offset.unwrap_or(0),
                    end_offset: ann.end_offset.unwrap_or(0),
                    element_id: None,
                    page_number: ann.page_number,
                    bounding_box: ann.bounding_box.clone(),
                    row_index: ann.row_index,
                    column_index: ann.column_index,
                    image_id: None,
                },
            );

            entities.push(entity);
        }

        Ok(entities)
    }
}
