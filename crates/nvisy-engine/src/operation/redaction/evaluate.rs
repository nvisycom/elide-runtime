//! Redaction operation.
//!
//! Runs at **phase 4** alongside [`GenerateContextOp`]. Evaluates policy
//! rules against detected entities to produce redaction records, then
//! builds and applies redaction instructions across all modalities
//! (text, image, audio) via [`RedactionApplicator`].
//!
//! [`GenerateContextOp`]: crate::operation::GenerateContextOp

use std::sync::Arc;

use nvisy_core::Result;
use nvisy_ontology::entity::{Entities, Entity, Location};
use nvisy_ontology::policy::{PolicyRule, RuleAction, Strategy, TextStrategy};
use nvisy_ontology::provenance::AuditEntry;
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
        let rules: Vec<PolicyRule> = shared.policies.all_rules().into_iter().cloned().collect();

        let default_spec = shared
            .policies
            .default_strategy()
            .cloned()
            .unwrap_or(Strategy::Text(TextStrategy::Mask { mask_char: '*' }));

        let evaluator = PolicyEvaluator {
            rules,
            default_spec,
            default_threshold: cfg.confidence_threshold.unwrap_or(0.5),
        };

        Self { evaluator }
    }
}

impl Operation for RedactionOp {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        if envelope.audit.entities.is_empty() {
            return Ok(());
        }

        tracing::debug!(
            target: TARGET,
            entities = envelope.audit.entities.len(),
            "evaluating redaction policies",
        );

        let document_labels = envelope.annotations.document_labels();
        let records = self
            .evaluator
            .evaluate(&envelope.audit.entities, &document_labels);
        envelope.audit.entries.extend(records);

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
    fn evaluate(&self, entities: &Entities, document_labels: &[&str]) -> Vec<AuditEntry> {
        tracing::debug!(
            target: TARGET,
            entity_count = entities.len(),
            rules = self.rules.len(),
            labels = document_labels.len(),
            "evaluating policies",
        );
        let mut records = Vec::new();

        for entity in entities {
            let rule = self.find_matching_rule(entity, document_labels);

            let spec = match rule {
                Some(r) => match &r.action {
                    RuleAction::Redact { strategy } => strategy.clone(),
                    _ => {
                        tracing::debug!(
                            target: TARGET,
                            entity_id = %entity.id,
                            rule_id = %r.id,
                            action = ?r.action,
                            "non-redact policy action",
                        );
                        continue;
                    }
                },
                None => {
                    if entity.confidence < self.default_threshold {
                        continue;
                    }
                    // Default strategy only applies to text entities.
                    // Non-text entities without a matching rule are skipped.
                    if !matches!(entity.location, Some(Location::Text(_))) {
                        continue;
                    }
                    self.default_spec.clone()
                }
            };

            let entity_id = entity.id;
            let policy_rule_id = rule.map(|r| r.id);

            let mut builder = AuditEntry::builder().for_entity(entity_id, spec, &entity.value);
            if let Some(rule_id) = policy_rule_id {
                builder = builder.with_policy_id(rule_id);
            }
            let record = builder.build().expect("all required fields set");

            tracing::trace!(
                target: TARGET,
                %entity_id,
                strategy = ?record.redaction.strategy,
                "produced redaction record",
            );

            records.push(record);
        }

        tracing::info!(
            target: TARGET,
            records = records.len(),
            "policy evaluation complete",
        );

        records
    }

    fn find_matching_rule(
        &self,
        entity: &Entity,
        document_labels: &[&str],
    ) -> Option<&PolicyRule> {
        self.rules.iter().find(|rule| {
            if !rule.enabled {
                return false;
            }
            if !rule.selector.matches(
                &entity.category,
                entity.entity_kind,
                entity.confidence,
                entity.sensitivity,
            ) {
                return false;
            }
            if let Some(ref conditions) = rule.conditions
                && !conditions
                    .required_labels
                    .iter()
                    .all(|required| document_labels.contains(&required.as_str()))
            {
                return false;
            }
            true
        })
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{Entity, EntityCategory, EntityKind, Location, RecognitionMethod};

    use super::*;

    fn test_entity(value: &str, confidence: f64) -> Entity {
        use nvisy_ontology::entity::TextLocation;

        Entity::builder()
            .with_category(EntityCategory::PersonalIdentity)
            .with_entity_kind(EntityKind::PersonName)
            .with_value(value)
            .with_recognition_methods(vec![RecognitionMethod::regex("test")])
            .with_confidence(confidence)
            .with_location(Location::from(TextLocation {
                start_offset: 0,
                end_offset: value.len(),
                span_index: Some(0),
                ..Default::default()
            }))
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
        let records = evaluator.evaluate(&entities, &[]);
        assert!(records.is_empty());
    }

    #[test]
    fn evaluator_produces_record_above_threshold() {
        let evaluator = PolicyEvaluator {
            rules: Vec::new(),
            default_spec: Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            default_threshold: 0.5,
        };
        let entities: Entities = vec![test_entity("John", 0.9)].into();
        let records = evaluator.evaluate(&entities, &[]);
        assert_eq!(records.len(), 1);
        assert!(!records[0].redaction.is_applied);
    }

    #[test]
    fn evaluator_uses_default_strategy_when_no_rules() {
        let evaluator = PolicyEvaluator {
            rules: Vec::new(),
            default_spec: Strategy::Text(TextStrategy::Remove),
            default_threshold: 0.0,
        };
        let entities: Entities = vec![test_entity("secret", 0.9)].into();
        let records = evaluator.evaluate(&entities, &[]);
        assert_eq!(
            records[0].redaction.strategy,
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
        let records = evaluator.evaluate(&entities, &[]);
        assert_eq!(records[0].value.original, "secret-value");
    }
}
