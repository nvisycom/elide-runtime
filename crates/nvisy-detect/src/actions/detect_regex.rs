use async_trait::async_trait;
use regex::Regex;
use std::any::Any;
use tokio::sync::mpsc;

use nvisy_core::data::DataValue;
use nvisy_core::datatypes::entity::{Entity, EntityLocation};
use nvisy_core::errors::NvisyError;
use nvisy_core::traits::action::Action;
use nvisy_core::types::DetectionMethod;

use crate::patterns::{self, PatternDefinition};

pub struct DetectRegexAction;

#[async_trait]
impl Action for DetectRegexAction {
    fn id(&self) -> &str {
        "detect-regex"
    }

    fn input_type(&self) -> &str {
        "document"
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
        let confidence_threshold: f64 = params
            .get("confidenceThreshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let requested_patterns: Option<Vec<String>> = params
            .get("patterns")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        // Resolve patterns
        let active_patterns = resolve_patterns(&requested_patterns);

        // Compile regexes
        let compiled: Vec<(&PatternDefinition, Regex)> = active_patterns
            .iter()
            .filter_map(|p| Regex::new(p.pattern_str).ok().map(|r| (*p, r)))
            .collect();

        let mut count = 0u64;

        while let Some(item) = input.recv().await {
            if let DataValue::Document(doc) = &item {
                for (pattern, regex) in &compiled {
                    for mat in regex.find_iter(&doc.content) {
                        let value = mat.as_str();

                        if let Some(validate) = pattern.validate {
                            if !validate(value) {
                                continue;
                            }
                        }

                        if pattern.confidence < confidence_threshold {
                            continue;
                        }

                        let mut entity = Entity::new(
                            pattern.category,
                            pattern.entity_type,
                            value,
                            DetectionMethod::Regex,
                            pattern.confidence,
                            EntityLocation {
                                start_offset: mat.start(),
                                end_offset: mat.end(),
                                element_id: None,
                                page_number: None,
                                bounding_box: None,
                            },
                        );
                        entity.source_id = Some(doc.data.id);
                        entity.data.parent_id = Some(doc.data.id);

                        count += 1;
                        if output.send(DataValue::Entity(entity)).await.is_err() {
                            return Ok(count);
                        }
                    }
                }
            }
        }

        Ok(count)
    }
}

fn resolve_patterns(requested: &Option<Vec<String>>) -> Vec<&'static PatternDefinition> {
    match requested {
        Some(names) if !names.is_empty() => names
            .iter()
            .filter_map(|n| patterns::get_pattern(n))
            .collect(),
        _ => patterns::get_all_patterns(),
    }
}
