//! Checksum-based entity validation action.

use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::ontology::entity::{DetectionMethod, Entity};
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::registry::action::Action;

use crate::patterns::validators::luhn_check;

/// Typed parameters for [`DetectChecksumAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectChecksumParams {
    /// Whether to discard entities that fail validation.
    #[serde(default = "default_true")]
    pub drop_invalid: bool,
    /// Amount added to confidence on successful validation.
    #[serde(default = "default_boost")]
    pub confidence_boost: f64,
}

fn default_true() -> bool { true }
fn default_boost() -> f64 { 0.05 }

/// Validates previously detected entities using checksum algorithms.
///
/// Entities whose type has a registered validator (e.g. Luhn for credit cards)
/// are verified. Valid matches receive a confidence boost and are re-emitted
/// with [`DetectionMethod::Checksum`]. Invalid matches can optionally be
/// dropped from the pipeline.
pub struct DetectChecksumAction;

#[async_trait::async_trait]
impl Action for DetectChecksumAction {
    type Params = DetectChecksumParams;

    fn id(&self) -> &str {
        "detect-checksum"
    }

    fn validate_params(&self, _params: &Self::Params) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        params: Self::Params,
    ) -> Result<u64, Error> {
        let drop_invalid = params.drop_invalid;
        let confidence_boost = params.confidence_boost;

        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            let entities: Vec<Entity> = blob.get_artifacts("entities").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read entities artifact: {e}"))
            })?;

            // Clear existing entities -- we will re-add validated ones
            blob.artifacts.remove("entities");

            for entity in entities {
                let validator = get_validator(&entity.entity_type);

                if let Some(validate) = validator {
                    let is_valid = validate(&entity.value);

                    if !is_valid && drop_invalid {
                        continue;
                    }

                    if is_valid {
                        let mut boosted = Entity::new(
                            entity.category,
                            &entity.entity_type,
                            &entity.value,
                            DetectionMethod::Checksum,
                            (entity.confidence + confidence_boost).min(1.0),
                            entity.location.clone(),
                        );
                        boosted.data.parent_id = entity.data.parent_id;
                        boosted.source_id = entity.source_id;

                        blob.add_artifact("entities", &boosted).map_err(|e| {
                            Error::new(ErrorKind::Runtime, format!("failed to add entity artifact: {e}"))
                        })?;

                        count += 1;
                        continue;
                    }
                }

                // No validator or not valid but not dropping -- pass through
                blob.add_artifact("entities", &entity).map_err(|e| {
                    Error::new(ErrorKind::Runtime, format!("failed to add entity artifact: {e}"))
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

/// Returns the checksum validator function for a given entity type, if one exists.
fn get_validator(entity_type: &str) -> Option<fn(&str) -> bool> {
    match entity_type {
        "credit_card" => Some(luhn_check),
        _ => None,
    }
}
