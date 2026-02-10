//! Policy evaluation action that maps detected entities to redaction instructions.

use std::any::Any;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::entity::Entity;
use nvisy_core::datatypes::policy::PolicyRule;
use nvisy_core::datatypes::redaction::Redaction;
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::traits::action::Action;
use nvisy_core::datatypes::redaction::RedactionMethod;

/// Evaluates policy rules against detected entities and emits [`Redaction`] artifacts.
///
/// For each entity the action finds the first matching rule (sorted by priority),
/// applies its redaction method and replacement template, and writes a
/// `"redactions"` artifact to the blob. Entities that fall below the confidence
/// threshold are skipped.
///
/// # Parameters (JSON)
///
/// | Key                          | Type                  | Default  | Description                                  |
/// |------------------------------|-----------------------|----------|----------------------------------------------|
/// | `rules`                      | `[PolicyRule]`        | `[]`     | Ordered policy rules to evaluate.            |
/// | `defaultMethod`              | `RedactionMethod`     | `Mask`   | Fallback redaction method when no rule matches.|
/// | `defaultConfidenceThreshold` | `f64`                 | `0.5`    | Fallback confidence threshold.               |
pub struct EvaluatePolicyAction;

#[async_trait::async_trait]
impl Action for EvaluatePolicyAction {
    fn id(&self) -> &str {
        "evaluate-policy"
    }

    fn validate_params(&self, _params: &serde_json::Value) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        params: serde_json::Value,
        _client: Option<Box<dyn Any + Send>>,
    ) -> Result<u64, Error> {
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

        while let Some(mut blob) = input.recv().await {
            let entities: Vec<Entity> = blob.get_artifacts("entities").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read entities artifact: {e}"))
            })?;

            for entity in &entities {
                let rule = find_matching_rule(entity, &sorted_rules);
                let method = rule.map(|r| r.method).unwrap_or(default_method);
                let threshold = rule
                    .map(|r| r.confidence_threshold)
                    .unwrap_or(default_threshold);

                if entity.confidence < threshold {
                    continue;
                }

                let replacement_value = if let Some(r) = rule {
                    apply_template(&r.replacement_template, entity)
                } else {
                    apply_default_mask(entity, default_method)
                };

                let mut redaction =
                    Redaction::new(entity.data.id, method, replacement_value);
                redaction = redaction.with_original_value(&entity.value);
                if let Some(r) = rule {
                    redaction = redaction.with_policy_rule_id(&r.id);
                }
                redaction.data.parent_id = Some(entity.data.id);

                blob.add_artifact("redactions", &redaction).map_err(|e| {
                    Error::new(ErrorKind::Runtime, format!("failed to add redaction artifact: {e}"))
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

/// Returns the first enabled rule whose category/entity-type filters and confidence
/// threshold match the given entity, or `None` if no rule applies.
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

/// Expands a replacement template using entity metadata.
///
/// Supported placeholders: `{entityType}`, `{category}`, `{value}`.
fn apply_template(template: &str, entity: &Entity) -> String {
    template
        .replace("{entityType}", &entity.entity_type)
        .replace(
            "{category}",
            &format!("{:?}", entity.category).to_lowercase(),
        )
        .replace("{value}", &entity.value)
}

/// Generates a replacement string for an entity using the given default redaction method.
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
