//! Policy evaluation: maps detected entities to redaction decisions.
//!
//! Applies configured [`PolicyRule`]s to each entity, producing a
//! [`RedactionDecision`] that specifies the redaction strategy and
//! replacement text for downstream application by [`Redaction`].
//!
//! [`PolicyRule`]: nvisy_ontology::policy::PolicyRule
//! [`RedactionDecision`]: crate::provenance::RedactionDecision
//! [`Redaction`]: super::Redaction

use nvisy_core::Result;
use nvisy_ontology::entity::{Entities, Entity};
use nvisy_ontology::policy::{PolicyRule, RuleAction, Strategy, TextStrategy};
use serde::Deserialize;

use crate::operation::{Operation, ParallelContext};
use crate::provenance::{RedactionDecision, RedactionRecord};

const TARGET: &str = "nvisy_engine::op::policy_evaluation";

/// Typed parameters for [`EvaluatePolicy`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluatePolicyParams {
    /// Ordered policy rules to evaluate.
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
    /// Fallback redaction strategy when no rule matches.
    #[serde(default = "default_spec")]
    pub default_spec: Strategy,
    /// Fallback confidence threshold.
    #[serde(default = "default_threshold")]
    pub default_confidence_threshold: f64,
}

fn default_spec() -> Strategy {
    Strategy::Text(TextStrategy::Mask { mask_char: '*' })
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
    pub async fn connect(mut params: EvaluatePolicyParams) -> Result<Self> {
        params.rules.sort_by_key(|r| r.priority);
        Ok(Self { params })
    }

    pub async fn execute(&self, entities: Entities) -> Result<EvaluatePolicyOutput> {
        tracing::debug!(target: TARGET, entity_count = entities.len(), "evaluating policies");
        let default_spec = &self.params.default_spec;
        let default_threshold = self.params.default_confidence_threshold;

        let mut decisions = Vec::new();
        let mut records = Vec::new();

        for entity in &entities {
            let rule = find_matching_rule(entity, &self.params.rules);

            let (spec, replacement) = match rule {
                Some(r) => match &r.action {
                    RuleAction::Redact { strategy } => (
                        strategy.clone(),
                        build_default_replacement(entity, strategy),
                    ),
                    RuleAction::Review
                    | RuleAction::Alert
                    | RuleAction::Block
                    | RuleAction::Suppress => continue,
                },
                None => {
                    if entity.confidence < default_threshold {
                        continue;
                    }
                    (
                        default_spec.clone(),
                        build_default_replacement(entity, default_spec),
                    )
                }
            };

            let entity_id = entity.source.as_uuid();

            let mut decision =
                RedactionDecision::new(entity_id, spec, replacement, entity.confidence);
            if let Some(r) = rule {
                decision = decision.with_policy_rule_id(r.id);
            }
            decision.source.set_parent_id(Some(entity_id));

            let mut record = RedactionRecord::new(entity_id, &entity.value, entity.confidence);
            if let Some(r) = rule {
                record = record.with_policy_rule_id(r.id);
            }
            record.source.set_parent_id(Some(entity_id));

            decisions.push(decision);
            records.push(record);
        }

        Ok(EvaluatePolicyOutput { decisions, records })
    }
}

impl Operation for EvaluatePolicy {
    type Input = ParallelContext<Entities>;
    type Output = ParallelContext<EvaluatePolicyOutput>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.execute(data)).await
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

/// Generates a replacement string for an entity using the given strategy.
fn build_default_replacement(entity: &Entity, spec: &Strategy) -> String {
    match spec {
        Strategy::Text(text) => match text {
            TextStrategy::Mask { mask_char } => mask_char.to_string().repeat(entity.value.len()),
            TextStrategy::Replace { placeholder } => {
                if placeholder.is_empty() {
                    format!("[{}]", entity.entity_kind.to_string().to_uppercase())
                } else {
                    placeholder
                        .replace("{entityType}", &entity.entity_kind.to_string())
                        .replace("{category}", &entity.category.to_string())
                        .replace("{value}", &entity.value)
                }
            }
            TextStrategy::Remove => String::new(),
            TextStrategy::Hash => format!("[HASH:{}]", entity.entity_kind),
            TextStrategy::Encrypt { .. } => format!("[ENC:{}]", entity.entity_kind),
            TextStrategy::Generate => format!("[GEN:{}]", entity.entity_kind),
            TextStrategy::Pseudonymize => format!("[PSEUDO:{}]", entity.entity_kind),
            TextStrategy::Tokenize { .. } => format!("[TOKEN:{}]", entity.entity_kind),
            TextStrategy::Aggregate => format!("[AGG:{}]", entity.entity_kind),
            TextStrategy::Generalize { .. } => format!("[GEN:{}]", entity.entity_kind),
        },
        Strategy::Image(_) | Strategy::Audio(_) => String::new(),
    }
}
