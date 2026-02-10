use async_trait::async_trait;
use std::any::Any;
use tokio::sync::mpsc;

use nvisy_core::data::DataValue;
use nvisy_core::datatypes::entity::Entity;
use nvisy_core::errors::NvisyError;
use nvisy_core::traits::action::Action;
use nvisy_core::types::DetectionMethod;

use crate::patterns::validators::luhn_check;

pub struct DetectChecksumAction;

#[async_trait]
impl Action for DetectChecksumAction {
    fn id(&self) -> &str {
        "detect-checksum"
    }

    fn input_type(&self) -> &str {
        "entity"
    }

    fn output_type(&self) -> &str {
        "entity"
    }

    fn validate_params(&self, _params: &serde_json::Value) -> Result<(), NvisyError> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<DataValue>,
        output: mpsc::Sender<DataValue>,
        params: serde_json::Value,
        _client: Option<Box<dyn Any + Send>>,
    ) -> Result<u64, NvisyError> {
        let drop_invalid = params
            .get("dropInvalid")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let confidence_boost = params
            .get("confidenceBoost")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.05);

        let mut count = 0u64;

        while let Some(item) = input.recv().await {
            if let DataValue::Entity(entity) = item {
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

                        count += 1;
                        if output.send(DataValue::Entity(boosted)).await.is_err() {
                            return Ok(count);
                        }
                        continue;
                    }
                }

                // No validator or not valid but not dropping — pass through
                count += 1;
                if output.send(DataValue::Entity(entity)).await.is_err() {
                    return Ok(count);
                }
            }
        }

        Ok(count)
    }
}

fn get_validator(entity_type: &str) -> Option<fn(&str) -> bool> {
    match entity_type {
        "credit_card" => Some(luhn_check),
        _ => None,
    }
}
