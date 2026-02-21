//! Checksum-based entity validation action.

use serde::Deserialize;

use crate::{DetectionMethod, Entity};
use nvisy_core::Error;

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

        let mut result = Vec::new();

        for entity in entities {
            let validator = get_validator(&entity.entity_type);

            if let Some(validate) = validator {
                let is_valid = validate(&entity.value);

                if !is_valid && drop_invalid {
                    continue;
                }

                if is_valid {
                    let mut boosted = Entity::new(
                        entity.category.clone(),
                        &entity.entity_type,
                        &entity.value,
                        DetectionMethod::Checksum,
                        (entity.confidence + confidence_boost).min(1.0),
                    );
                    boosted.copy_locations_from(&entity);
                    boosted.source.set_parent_id(entity.source.parent_id());

                    result.push(boosted);
                    continue;
                }
            }

            // No validator or not valid but not dropping -- pass through
            result.push(entity);
        }

        Ok(result)
    }
}

/// Returns the checksum validator function for a given entity type, if one exists.
fn get_validator(entity_type: &str) -> Option<fn(&str) -> bool> {
    match entity_type {
        "credit_card" => Some(luhn_check),
        _ => None,
    }
}

/// Luhn checksum for credit/debit card numbers.
///
/// Strips non-digit characters, then validates per ISO/IEC 7812.
fn luhn_check(num: &str) -> bool {
    let digits: Vec<u32> = num.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.is_empty() {
        return false;
    }
    let mut sum = 0u32;
    let mut alternate = false;
    for &d in digits.iter().rev() {
        let mut n = d;
        if alternate {
            n *= 2;
            if n > 9 {
                n -= 9;
            }
        }
        sum += n;
        alternate = !alternate;
    }
    sum % 10 == 0
}
