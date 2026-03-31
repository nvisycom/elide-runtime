//! Redaction operation.
//!
//! Runs at **phase 4** alongside [`GenerateContextOp`]. Evaluates policy
//! rules against detected entities to produce redaction decisions.
//! The actual replacement text or codec instructions are computed at
//! application time by the executor, not here.
//!
//! [`GenerateContextOp`]: crate::operation::GenerateContextOp

use nvisy_core::Result;
use nvisy_ontology::entity::{Entities, Entity};
use nvisy_ontology::policy::{PolicyRule, RuleAction, Strategy, TextStrategy};
use nvisy_ontology::provenance::{RedactionDecision, RedactionRecord};
use nvisy_ontology::workflow::Redaction;

use crate::operation::Operation;
use crate::operation::context::{ParallelContext, SharedContext};
use crate::operation::envelope::PolicyOutcome;

const TARGET: &str = "nvisy_engine::op::redaction";

/// Redaction operation: evaluates policies and produces redaction decisions.
pub struct RedactionOp {
    evaluator: PolicyEvaluator,
}

impl RedactionOp {
    /// Build from graph config and shared context.
    pub async fn new(cfg: &Redaction, shared: &SharedContext) -> Result<Self> {
        let mut rules: Vec<PolicyRule> = shared
            .policies
            .policies
            .iter()
            .flat_map(|p| p.rules.clone())
            .collect();
        rules.sort_by_key(|r| r.priority);

        let evaluator = PolicyEvaluator {
            rules,
            default_spec: Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            default_threshold: cfg.confidence_threshold.unwrap_or(0.5),
        };

        Ok(Self { evaluator })
    }
}

impl Operation for RedactionOp {
    type Input = ParallelContext<Entities>;
    type Output = ParallelContext<PolicyOutcome>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input
            .parallel_map(|data| async { self.evaluator.evaluate(data) })
            .await
    }
}

struct PolicyEvaluator {
    rules: Vec<PolicyRule>,
    default_spec: Strategy,
    default_threshold: f64,
}

impl PolicyEvaluator {
    fn evaluate(&self, entities: Entities) -> Result<PolicyOutcome> {
        tracing::debug!(
            target: TARGET,
            entity_count = entities.len(),
            rules = self.rules.len(),
            "evaluating policies",
        );
        let mut decisions = Vec::new();
        let mut records = Vec::new();

        for entity in &entities {
            let rule = self.find_matching_rule(entity);

            let spec = match rule {
                Some(r) => match &r.action {
                    RuleAction::Redact { strategy } => strategy.clone(),
                    action @ (RuleAction::Review
                    | RuleAction::Alert
                    | RuleAction::Block
                    | RuleAction::Suppress
                    | _) => {
                        tracing::debug!(
                            target: TARGET,
                            entity_id = %entity.source.as_uuid(),
                            rule_id = %r.id,
                            action = ?action,
                            "non-redact policy action",
                        );
                        continue;
                    }
                },
                None => {
                    if entity.confidence < self.default_threshold {
                        continue;
                    }
                    self.default_spec.clone()
                }
            };

            let entity_id = entity.source.as_uuid();

            let mut decision = RedactionDecision::new(entity_id, spec, entity.confidence);
            if let Some(r) = rule {
                decision = decision.with_policy_rule_id(r.id);
            }
            decision.source.set_parent_id(Some(entity_id));

            let mut record = RedactionRecord::new(entity_id, &entity.value, entity.confidence);
            if let Some(r) = rule {
                record = record.with_policy_rule_id(r.id);
            }
            record.source.set_parent_id(Some(entity_id));

            tracing::trace!(
                target: TARGET,
                entity_id = %entity_id,
                strategy = ?decision.spec,
                "produced redaction decision",
            );

            decisions.push(decision);
            records.push(record);
        }

        tracing::info!(
            target: TARGET,
            decisions = decisions.len(),
            records = records.len(),
            "policy evaluation complete",
        );

        Ok(PolicyOutcome { decisions, records })
    }

    fn find_matching_rule(&self, entity: &Entity) -> Option<&PolicyRule> {
        self.rules.iter().find(|rule| {
            rule.selector
                .matches(&entity.category, entity.entity_kind, entity.confidence)
        })
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{Entity, EntityCategory, EntityKind, RecognitionMethod};

    use super::*;

    fn test_entity(value: &str, confidence: f64) -> Entity {
        Entity::builder()
            .with_category(EntityCategory::PersonalIdentity)
            .with_entity_kind(EntityKind::PersonName)
            .with_value(value)
            .with_recognition_methods(vec![RecognitionMethod::regex("test")])
            .with_confidence(confidence)
            .build()
            .unwrap()
    }

    #[test]
    fn evaluator_skips_below_threshold() {
        let evaluator = PolicyEvaluator {
            rules: Vec::new(),
            default_spec: Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            default_threshold: 0.8,
        };
        let entities: Entities = vec![test_entity("John", 0.5)].into();
        let result = evaluator.evaluate(entities).unwrap();
        assert!(result.decisions.is_empty());
    }

    #[test]
    fn evaluator_produces_decision_above_threshold() {
        let evaluator = PolicyEvaluator {
            rules: Vec::new(),
            default_spec: Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            default_threshold: 0.5,
        };
        let entities: Entities = vec![test_entity("John", 0.9)].into();
        let result = evaluator.evaluate(entities).unwrap();
        assert_eq!(result.decisions.len(), 1);
        assert_eq!(result.records.len(), 1);
        assert!(!result.decisions[0].applied);
    }

    #[test]
    fn evaluator_uses_default_strategy_when_no_rules() {
        let evaluator = PolicyEvaluator {
            rules: Vec::new(),
            default_spec: Strategy::Text(TextStrategy::Remove),
            default_threshold: 0.0,
        };
        let entities: Entities = vec![test_entity("secret", 0.9)].into();
        let result = evaluator.evaluate(entities).unwrap();
        assert_eq!(
            result.decisions[0].spec,
            Strategy::Text(TextStrategy::Remove)
        );
    }

    #[test]
    fn record_captures_original_value() {
        let evaluator = PolicyEvaluator {
            rules: Vec::new(),
            default_spec: Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            default_threshold: 0.0,
        };
        let entities: Entities = vec![test_entity("secret-value", 0.9)].into();
        let result = evaluator.evaluate(entities).unwrap();
        assert_eq!(result.records[0].original_value, "secret-value");
    }
}
