use async_trait::async_trait;
use std::any::Any;
use tokio::sync::mpsc;

use nvisy_core::data::DataValue;
use nvisy_core::datatypes::entity::Entity;
use nvisy_core::datatypes::policy::PolicyRule;
use nvisy_core::datatypes::redaction::Redaction;
use nvisy_core::errors::NvisyError;
use nvisy_core::traits::action::Action;
use nvisy_core::types::RedactionMethod;

pub struct EvaluatePolicyAction;

#[async_trait]
impl Action for EvaluatePolicyAction {
    fn id(&self) -> &str {
        "evaluate-policy"
    }

    fn input_type(&self) -> &str {
        "entity"
    }

    fn output_type(&self) -> &str {
        "redaction"
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
        let rules: Vec<PolicyRule> = params
            .get("rules")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let default_method: RedactionMethod = params
            .get("defaultMethod")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or(RedactionMethod::Mask);
        let default_threshold: f64 = params
            .get("defaultConfidenceThreshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);

        let mut sorted_rules = rules;
        sorted_rules.sort_by_key(|r| r.priority);

        let mut count = 0u64;

        while let Some(item) = input.recv().await {
            if let DataValue::Entity(entity) = item {
                let rule = find_matching_rule(&entity, &sorted_rules);
                let method = rule.map(|r| r.method).unwrap_or(default_method);
                let threshold = rule
                    .map(|r| r.confidence_threshold)
                    .unwrap_or(default_threshold);

                if entity.confidence < threshold {
                    continue;
                }

                let replacement_value = if let Some(r) = rule {
                    apply_template(&r.replacement_template, &entity)
                } else {
                    apply_default_mask(&entity, default_method)
                };

                let mut redaction =
                    Redaction::new(entity.data.id, method, replacement_value);
                redaction = redaction.with_original_value(&entity.value);
                if let Some(r) = rule {
                    redaction = redaction.with_policy_rule_id(&r.id);
                }
                redaction.data.parent_id = Some(entity.data.id);

                count += 1;
                if output.send(DataValue::Redaction(redaction)).await.is_err() {
                    return Ok(count);
                }
            }
        }

        Ok(count)
    }
}

fn find_matching_rule<'a>(entity: &Entity, rules: &'a [PolicyRule]) -> Option<&'a PolicyRule> {
    for rule in rules {
        if !rule.enabled {
            continue;
        }
        if entity.confidence < rule.confidence_threshold {
            continue;
        }
        if !rule.categories.is_empty() && !rule.categories.contains(&entity.category) {
            continue;
        }
        if !rule.entity_types.is_empty()
            && !rule.entity_types.iter().any(|t| t == &entity.entity_type)
        {
            continue;
        }
        return Some(rule);
    }
    None
}

fn apply_template(template: &str, entity: &Entity) -> String {
    template
        .replace("{entityType}", &entity.entity_type)
        .replace(
            "{category}",
            &format!("{:?}", entity.category).to_lowercase(),
        )
        .replace("{value}", &entity.value)
}

fn apply_default_mask(entity: &Entity, method: RedactionMethod) -> String {
    match method {
        RedactionMethod::Mask => "*".repeat(entity.value.len()),
        RedactionMethod::Replace => format!("[{}]", entity.entity_type.to_uppercase()),
        RedactionMethod::Remove => String::new(),
        RedactionMethod::Hash => format!("[HASH:{}]", entity.entity_type),
        RedactionMethod::Encrypt => format!("[ENC:{}]", entity.entity_type),
        RedactionMethod::Blur => format!("[BLURRED:{}]", entity.entity_type),
        RedactionMethod::Block => "\u{2588}".repeat(entity.value.len()),
        RedactionMethod::Synthesize => format!("[SYNTH:{}]", entity.entity_type),
    }
}
