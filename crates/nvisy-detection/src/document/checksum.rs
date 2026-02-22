//! Checksum-based entity validation action.

use serde::Deserialize;

use crate::{DetectionMethod, Entity};
use nvisy_core::Error;
use nvisy_pattern::default_engine;

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

fn default_true() -> bool {
    true
}
fn default_boost() -> f64 {
    0.05
}

/// Validates previously detected entities using checksum algorithms.
///
/// Entities whose type has a registered validator (e.g. Luhn for credit cards)
/// are verified. Valid matches receive a confidence boost and are re-emitted
/// with [`DetectionMethod::Checksum`]. Invalid matches can optionally be
/// dropped from the pipeline.
pub struct DetectChecksumAction {
    params: DetectChecksumParams,
}

impl DetectChecksumAction {
    pub async fn connect(params: DetectChecksumParams) -> Result<Self, Error> {
        Ok(Self { params })
    }

    pub async fn execute(
        &self,
        entities: Vec<Entity>,
    ) -> Result<Vec<Entity>, Error> {
        let drop_invalid = self.params.drop_invalid;
        let confidence_boost = self.params.confidence_boost;
        let engine = default_engine();

        let mut result = Vec::new();

        for entity in entities {
            match engine.validate_checksum(entity.entity_kind, &entity.value) {
                Some(true) => {
                    let mut boosted = Entity::new(
                        entity.category.clone(),
                        entity.entity_kind,
                        &entity.value,
                        DetectionMethod::Checksum,
                        (entity.confidence + confidence_boost).min(1.0),
                    );
                    boosted.copy_locations_from(&entity);
                    boosted.source.set_parent_id(entity.source.parent_id());
                    result.push(boosted);
                }
                Some(false) if drop_invalid => continue,
                _ => result.push(entity),
            }
        }

        Ok(result)
    }
}
