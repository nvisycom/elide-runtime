//! Policy evaluation: maps detected entities to redaction instructions.

use nvisy_core::Error;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::policy::{PolicyRule, RuleAction};
use nvisy_ontology::record::{RedactionDecision, RedactionRecord};
use nvisy_ontology::policy::{RedactionStrategy, TextRedactionStrategy};
use serde::Deserialize;

use crate::operation::{Operation, ParallelContext};

/// Typed parameters for [`EvaluatePolicy`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluatePolicyParams {
    /// Ordered policy rules to evaluate.
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
    /// Fallback redaction strategy when no rule matches.
    #[serde(default = "default_spec")]
    pub default_spec: RedactionStrategy,
    /// Fallback confidence threshold.
    #[serde(default = "default_threshold")]
    pub default_confidence_threshold: f64,
}

fn default_spec() -> RedactionStrategy {
    RedactionStrategy::Text(TextRedactionStrategy::Mask { mask_char: '*' })
}
fn default_threshold() -> f64 {
    0.5
}

/// Output of policy evaluation: both pipeline decisions and audit records.
pub struct EvaluatePolicyOutput {
    /// Pipeline-facing redaction decisions.
    pub decisions: Vec<RedactionDecision>,
    /// Audit-facing redaction records.
    pub records: Vec<RedactionRecord>,
}

/// Evaluates policy rules against detected entities and produces
/// [`RedactionDecision`] and [`RedactionRecord`] pairs.
///
/// For each entity the action finds the first matching rule (sorted by priority),
/// applies its redaction strategy and replacement template, and creates both a
/// decision and an audit record. Entities below the confidence threshold are skipped.
pub struct EvaluatePolicy {
    params: EvaluatePolicyParams,
}

impl EvaluatePolicy {
    pub async fn connect(mut params: EvaluatePolicyParams) -> Result<Self, Error> {
        params.rules.sort_by_key(|r| r.priority);
        Ok(Self { params })
    }

    pub async fn execute(&self, entities: Vec<Entity>) -> Result<EvaluatePolicyOutput, Error> {
        let default_spec = &self.params.default_spec;
        let default_threshold = self.params.default_confidence_threshold;

        let mut decisions = Vec::new();
        let mut records = Vec::new();

        for entity in &entities {
            let rule = find_matching_rule(entity, &self.params.rules);

            let (spec, replacement) = match rule {
                Some(r) => match &r.action {
                    RuleAction::Redact {
                        strategy,
                        replacement_template,
                    } => (strategy.clone(), apply_template(replacement_template, entity)),
                    RuleAction::Review | RuleAction::Alert | RuleAction::Block | RuleAction::Suppress => continue,
                },
                None => {
                    if entity.confidence < default_threshold {
                        continue;
                    }
                    (default_spec.clone(), build_default_replacement(entity, default_spec))
                }
            };

            let entity_id = entity.source.as_uuid();

            let mut decision = RedactionDecision::new(
                entity_id,
                spec,
                replacement,
                entity.confidence,
            );
            if let Some(r) = rule {
                decision = decision.with_policy_rule_id(r.id);
            }
            decision
                .source
                .set_parent_id(Some(entity_id));

            let mut record = RedactionRecord::new(
                entity_id,
                &entity.value,
                entity.confidence,
            );
            if let Some(r) = rule {
                record = record.with_policy_rule_id(r.id);
            }
            record
                .source
                .set_parent_id(Some(entity_id));

            decisions.push(decision);
            records.push(record);
        }

        Ok(EvaluatePolicyOutput { decisions, records })
    }
}

impl Operation for EvaluatePolicy {
    type Input = ParallelContext<Vec<Entity>>;
    type Output = ParallelContext<EvaluatePolicyOutput>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output, Error> {
        let result = self.execute(input.into_inner()).await?;
        Ok(ParallelContext::new(result))
    }
}

/// Returns the first enabled rule whose [`EntitySelector`] matches the given entity,
/// or `None` if no rule applies.
fn find_matching_rule<'a>(entity: &Entity, rules: &'a [PolicyRule]) -> Option<&'a PolicyRule> {
    rules.iter().find(|rule| {
        rule.selector
            .matches(&entity.category, entity.entity_kind, entity.confidence)
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

/// Generates a default replacement string for an entity using the given strategy.
fn build_default_replacement(entity: &Entity, spec: &RedactionStrategy) -> String {
    match spec {
        RedactionStrategy::Text(text) => match text {
            TextRedactionStrategy::Mask { mask_char } => {
                mask_char.to_string().repeat(entity.value.len())
            }
            TextRedactionStrategy::Replace { placeholder } => {
                if placeholder.is_empty() {
                    format!("[{}]", entity.entity_kind.to_string().to_uppercase())
                } else {
                    apply_template(placeholder, entity)
                }
            }
            TextRedactionStrategy::Remove => String::new(),
            TextRedactionStrategy::Hash => format!("[HASH:{}]", entity.entity_kind),
            TextRedactionStrategy::Encrypt { .. } => format!("[ENC:{}]", entity.entity_kind),
            TextRedactionStrategy::Generate => format!("[GEN:{}]", entity.entity_kind),
            TextRedactionStrategy::Pseudonymize => format!("[PSEUDO:{}]", entity.entity_kind),
            TextRedactionStrategy::Tokenize { .. } => format!("[TOKEN:{}]", entity.entity_kind),
            TextRedactionStrategy::Aggregate => format!("[AGG:{}]", entity.entity_kind),
            TextRedactionStrategy::Generalize { .. } => format!("[GEN:{}]", entity.entity_kind),
        },
        RedactionStrategy::Image(_) | RedactionStrategy::Audio(_) => String::new(),
    }
}
