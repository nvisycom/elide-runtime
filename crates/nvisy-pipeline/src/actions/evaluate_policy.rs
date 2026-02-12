//! Policy evaluation action that maps detected entities to redaction instructions.

use serde::Deserialize;

use nvisy_ontology::ontology::entity::Entity;
use nvisy_ontology::redaction::policy::PolicyRule;
use nvisy_ontology::ontology::redaction::{Redaction, RedactionMethod};
use nvisy_core::error::Error;

use crate::action::Action;

/// Typed parameters for [`EvaluatePolicyAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluatePolicyParams {
    /// Ordered policy rules to evaluate.
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
    /// Fallback redaction method when no rule matches.
    #[serde(default = "default_method")]
    pub default_method: RedactionMethod,
    /// Fallback confidence threshold.
    #[serde(default = "default_threshold")]
    pub default_confidence_threshold: f64,
}

fn default_method() -> RedactionMethod {
    RedactionMethod::Mask
}
fn default_threshold() -> f64 {
    0.5
}

/// Evaluates policy rules against detected entities and produces [`Redaction`] instructions.
///
/// For each entity the action finds the first matching rule (sorted by priority),
/// applies its redaction method and replacement template, and creates a
/// [`Redaction`]. Entities that fall below the confidence threshold are skipped.
pub struct EvaluatePolicyAction {
    params: EvaluatePolicyParams,
}

#[async_trait::async_trait]
impl Action for EvaluatePolicyAction {
    type Params = EvaluatePolicyParams;
    type Input = Vec<Entity>;
    type Output = Vec<Redaction>;

    fn id(&self) -> &str {
        "evaluate-policy"
    }

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        Ok(Self { params })
    }

    async fn execute(
        &self,
        entities: Self::Input,
    ) -> Result<Vec<Redaction>, Error> {
        let default_method = self.params.default_method;
        let default_threshold = self.params.default_confidence_threshold;

        let mut sorted_rules = self.params.rules.clone();
        sorted_rules.sort_by_key(|r| r.priority);

        let mut redactions = Vec::new();

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
                Redaction::new(entity.source.as_uuid(), method, replacement_value);
            redaction = redaction.with_original_value(&entity.value);
            if let Some(r) = rule {
                redaction = redaction.with_policy_rule_id(&r.id);
            }
            redaction.source.set_parent_id(Some(entity.source.as_uuid()));

            redactions.push(redaction);
        }

        Ok(redactions)
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
