//! Redaction operation.
//!
//! Runs at **phase 4** alongside [`GenerateContextOp`]. Evaluates policy
//! rules against detected entities to produce redaction decisions, then
//! builds and applies redaction instructions across all modalities
//! (text, image, audio) via [`RedactionApplicator`].
//!
//! [`GenerateContextOp`]: crate::operation::GenerateContextOp

use std::sync::Arc;

use nvisy_core::Result;
use nvisy_ontology::entity::{Entities, Entity};
use nvisy_ontology::policy::{PolicyRule, RuleAction, Strategy, TextStrategy};
use nvisy_ontology::provenance::{RedactionDecision, RedactionRecord};
use nvisy_ontology::workflow::Redaction;

use super::apply::RedactionApplicator;
use crate::operation::envelope::SharedData;
use crate::operation::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::redaction";

/// Redaction operation: evaluates policies and applies redaction instructions.
pub struct RedactionOp {
    evaluator: PolicyEvaluator,
}

impl RedactionOp {
    /// Build from graph config and shared context.
    pub fn new(cfg: &Redaction, shared: &Arc<SharedData>) -> Self {
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

        Self { evaluator }
    }
}

impl Operation for RedactionOp {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        if envelope.entities.is_empty() {
            return Ok(());
        }

        tracing::debug!(
            target: TARGET,
            entities = envelope.entities.len(),
            "evaluating redaction policies",
        );

        let (decisions, records) = self.evaluator.evaluate(&envelope.entities)?;
        envelope.audit.decisions.extend(decisions);
        envelope.audit.records.extend(records);

        RedactionApplicator::new(envelope).apply().await?;

        Ok(())
    }
}

struct PolicyEvaluator {
    rules: Vec<PolicyRule>,
    default_spec: Strategy,
    default_threshold: f64,
}

impl PolicyEvaluator {
    fn evaluate(
        &self,
        entities: &Entities,
    ) -> Result<(Vec<RedactionDecision>, Vec<RedactionRecord>)> {
        tracing::debug!(
            target: TARGET,
            entity_count = entities.len(),
            rules = self.rules.len(),
            "evaluating policies",
        );
        let mut decisions = Vec::new();
        let mut records = Vec::new();

        for entity in entities {
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

        Ok((decisions, records))
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
        let (decisions, _) = evaluator.evaluate(&entities).unwrap();
        assert!(decisions.is_empty());
    }

    #[test]
    fn evaluator_produces_decision_above_threshold() {
        let evaluator = PolicyEvaluator {
            rules: Vec::new(),
            default_spec: Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            default_threshold: 0.5,
        };
        let entities: Entities = vec![test_entity("John", 0.9)].into();
        let (decisions, records) = evaluator.evaluate(&entities).unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(records.len(), 1);
        assert!(!decisions[0].applied);
    }

    #[test]
    fn evaluator_uses_default_strategy_when_no_rules() {
        let evaluator = PolicyEvaluator {
            rules: Vec::new(),
            default_spec: Strategy::Text(TextStrategy::Remove),
            default_threshold: 0.0,
        };
        let entities: Entities = vec![test_entity("secret", 0.9)].into();
        let (decisions, _) = evaluator.evaluate(&entities).unwrap();
        assert_eq!(decisions[0].spec, Strategy::Text(TextStrategy::Remove));
    }

    #[test]
    fn record_captures_original_value() {
        let evaluator = PolicyEvaluator {
            rules: Vec::new(),
            default_spec: Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            default_threshold: 0.0,
        };
        let entities: Entities = vec![test_entity("secret-value", 0.9)].into();
        let (_, records) = evaluator.evaluate(&entities).unwrap();
        assert_eq!(records[0].original_value, "secret-value");
    }
}
