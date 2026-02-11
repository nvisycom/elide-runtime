//! Manual annotation detection action.
//!
//! Converts user-provided [`ManualAnnotation`]s from the blob's
//! `"manual_entities"` artifact into full [`Entity`] objects.

use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_ontology::ontology::entity::{DetectionMethod, Entity, EntityLocation};
use nvisy_ontology::redaction::ManualAnnotation;
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::registry::action::Action;

/// Typed parameters for [`DetectManualAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectManualParams {}

/// Reads `"manual_entities"` artifacts from the blob (injected by the
/// server from `RedactionContext.manual_entities`) and converts each
/// [`ManualAnnotation`] into a full [`Entity`] with
/// `DetectionMethod::Manual` and confidence 1.0.
pub struct DetectManualAction;

#[async_trait::async_trait]
impl Action for DetectManualAction {
    type Params = DetectManualParams;

    fn id(&self) -> &str {
        "detect-manual"
    }

    fn validate_params(&self, _params: &Self::Params) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        _params: Self::Params,
    ) -> Result<u64, Error> {
        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            let annotations: Vec<ManualAnnotation> =
                blob.get_artifacts("manual_entities").map_err(|e| {
                    Error::new(
                        ErrorKind::Runtime,
                        format!("failed to read manual_entities artifact: {e}"),
                    )
                })?;

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

                blob.add_artifact("entities", &entity).map_err(|e| {
                    Error::new(ErrorKind::Runtime, format!("failed to add entity: {e}"))
                })?;
                count += 1;
            }

            if output.send(blob).await.is_err() {
                return Ok(count);
            }
        }

        Ok(count)
    }
}
