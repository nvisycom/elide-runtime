//! Policy evaluation action that maps detected entities to redaction instructions.

use serde::Deserialize;

use nvisy_detection::Entity;
use crate::record::Redaction;
use crate::rule::PolicyRule;
use crate::spec::RedactionSpec;
use crate::text::spec::TextRedactionSpec;
use nvisy_core::Error;

/// Typed parameters for [`EvaluatePolicyAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluatePolicyParams {
    /// Ordered policy rules to evaluate.
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
    /// Fallback redaction specification when no rule matches.
    #[serde(default = "default_spec")]
    pub default_spec: RedactionSpec,
    /// Fallback confidence threshold.
    #[serde(default = "default_threshold")]
    pub default_confidence_threshold: f64,
}

fn default_spec() -> RedactionSpec {
    RedactionSpec::Text(TextRedactionSpec::Mask { mask_char: '*' })
}
fn default_threshold() -> f64 {
    0.5
}

/// Evaluates policy rules against detected entities and produces [`Redaction`] instructions.
///
/// For each entity the action finds the first matching rule (sorted by priority),
/// applies its redaction spec and replacement template, and creates a
/// [`Redaction`]. Entities that fall below the confidence threshold are skipped.
pub struct EvaluatePolicyAction {
    params: EvaluatePolicyParams,
}

impl EvaluatePolicyAction {
    pub async fn connect(mut params: EvaluatePolicyParams) -> Result<Self, Error> {
        params.rules.sort_by_key(|r| r.priority);
        Ok(Self { params })
    }

    pub async fn execute(
        &self,
        entities: Vec<Entity>,
    ) -> Result<Vec<Redaction>, Error> {
        let default_spec = &self.params.default_spec;
        let default_threshold = self.params.default_confidence_threshold;

        let mut redactions = Vec::new();

        for entity in &entities {
            let rule = find_matching_rule(entity, &self.params.rules);
            let spec = rule.map(|r| &r.spec).unwrap_or(default_spec);

            if rule.is_none() && entity.confidence < default_threshold {
                continue;
            }

            let replacement = if let Some(r) = rule {
                apply_template(&r.replacement_template, entity)
            } else {
                build_default_replacement(entity, spec)
            };

            let mut redaction = Redaction::new(
                entity.source.as_uuid(),
                spec.clone(),
                replacement,
                &entity.value,
                entity.confidence,
            );
            if let Some(r) = rule {
                redaction = redaction.with_policy_rule_id(r.id);
            }
            redaction.source.set_parent_id(Some(entity.source.as_uuid()));

            redactions.push(redaction);
        }

        Ok(redactions)
    }
}

/// Returns the first enabled rule whose [`EntitySelector`] matches the given entity,
/// or `None` if no rule applies.
fn find_matching_rule<'a>(entity: &Entity, rules: &'a [PolicyRule]) -> Option<&'a PolicyRule> {
    rules.iter().find(|rule| {
        rule.selector.matches(&entity.category, entity.entity_kind, entity.confidence)
    })
}

/// Expands a replacement template using entity metadata.
///
/// Supported placeholders: `{entityType}`, `{category}`, `{value}`.
fn apply_template(template: &str, entity: &Entity) -> String {
    template
        .replace("{entityType}", &entity.entity_kind.to_string())
        .replace("{category}", &entity.category.to_string())
        .replace("{value}", &entity.value)
}

/// Generates a default replacement string for an entity using the given redaction spec.
fn build_default_replacement(entity: &Entity, spec: &RedactionSpec) -> String {
    match spec {
        RedactionSpec::Text(text) => match text {
            TextRedactionSpec::Mask { mask_char } => {
                mask_char.to_string().repeat(entity.value.len())
            }
            TextRedactionSpec::Replace { placeholder } => {
                if placeholder.is_empty() {
                    format!("[{}]", entity.entity_kind.to_string().to_uppercase())
                } else {
                    apply_template(placeholder, entity)
                }
            }
            TextRedactionSpec::Remove => String::new(),
            TextRedactionSpec::Hash => format!("[HASH:{}]", entity.entity_kind),
            TextRedactionSpec::Encrypt { .. } => format!("[ENC:{}]", entity.entity_kind),
            TextRedactionSpec::Synthesize => format!("[SYNTH:{}]", entity.entity_kind),
            TextRedactionSpec::Pseudonymize => format!("[PSEUDO:{}]", entity.entity_kind),
            TextRedactionSpec::Tokenize { .. } => format!("[TOKEN:{}]", entity.entity_kind),
            TextRedactionSpec::Aggregate => format!("[AGG:{}]", entity.entity_kind),
            TextRedactionSpec::Generalize { .. } => format!("[GEN:{}]", entity.entity_kind),
            TextRedactionSpec::DateShift { .. } => format!("[SHIFTED:{}]", entity.entity_kind),
        },
        // Image and audio specs don't produce text replacements.
        RedactionSpec::Image(_) | RedactionSpec::Audio(_) => String::new(),
    }
}
