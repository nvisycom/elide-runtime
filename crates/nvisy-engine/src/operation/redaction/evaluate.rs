//! Redaction operation.
//!
//! Runs at **phase 4** alongside [`GenerateContextOp`]. Evaluates policy
//! rules against detected entities to produce redaction records, then
//! builds and applies redaction instructions across all modalities
//! (text, image, audio) via [`RedactionApplicator`].
//!
//! [`GenerateContextOp`]: crate::operation::GenerateContextOp

use nvisy_core::Result;
use nvisy_ontology::entity::{Entities, Entity};
use nvisy_ontology::policy::{RuleAction, RuleCondition, Strategy, StrategyPolicy, TextStrategy};
use nvisy_ontology::provenance::AuditEntry;
use nvisy_ontology::workflow::Redaction;
use uuid::Uuid;

use super::apply::RedactionApplicator;
use crate::operation::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::redaction";

/// Redaction operation: evaluates policies and applies redaction instructions.
pub struct RedactionOp {
    default_threshold: f64,
}

impl RedactionOp {
    /// Build from graph config.
    pub fn new(cfg: &Redaction) -> Self {
        Self {
            default_threshold: cfg.confidence_threshold.unwrap_or(0.5),
        }
    }
}

impl Operation for RedactionOp {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        if envelope.audit.entities.is_empty() {
            return Ok(());
        }

        let policies = &envelope.shared.policies;
        let strategies = policies.all_strategies();
        let default_spec = policies
            .default_strategy()
            .cloned()
            .unwrap_or(Strategy::Text(TextStrategy::Mask { mask_char: '*' }));

        tracing::debug!(
            target: TARGET,
            entities = envelope.audit.entities.len(),
            strategies = strategies.len(),
            "evaluating redaction policies",
        );

        let document_labels = envelope.annotations.document_labels();
        let entries = evaluate(
            &envelope.audit.entities,
            &strategies,
            &default_spec,
            self.default_threshold,
            &document_labels,
        );
        envelope.audit.entries.extend(entries);

        RedactionApplicator::new(envelope).apply().await?;

        Ok(())
    }
}

/// Evaluate strategy policies against entities, producing audit entries.
fn evaluate(
    entities: &Entities,
    strategies: &[(Uuid, &StrategyPolicy)],
    default_spec: &Strategy,
    default_threshold: f64,
    document_labels: &[&str],
) -> Vec<AuditEntry> {
    let mut entries = Vec::new();

    for entity in entities {
        let matched = find_matching_strategy(strategies, entity, document_labels);

        let (spec, policy_id) = match matched {
            Some((policy_id, strategy)) => match &strategy.action {
                RuleAction::Redact { strategy } => (strategy.clone(), Some(policy_id)),
                _ => {
                    tracing::debug!(
                        target: TARGET,
                        entity_id = %entity.id,
                        %policy_id,
                        action = ?strategy.action,
                        "non-redact policy action",
                    );
                    continue;
                }
            },
            None => {
                if entity.confidence < default_threshold {
                    continue;
                }
                (default_spec.clone(), None)
            }
        };

        let entity_id = entity.id;
        let mut builder = AuditEntry::builder().for_entity(entity_id, spec, &entity.value);
        if let Some(id) = policy_id {
            builder = builder.with_policy_id(id);
        }
        let entry = builder.build().expect("all required fields set");

        tracing::trace!(
            target: TARGET,
            %entity_id,
            strategy = ?entry.redaction.strategy,
            "produced audit entry",
        );

        entries.push(entry);
    }

    tracing::info!(
        target: TARGET,
        entries = entries.len(),
        "policy evaluation complete",
    );

    entries
}

fn find_matching_strategy<'a>(
    strategies: &[(Uuid, &'a StrategyPolicy)],
    entity: &Entity,
    document_labels: &[&str],
) -> Option<(Uuid, &'a StrategyPolicy)> {
    strategies
        .iter()
        .find(|(_, strategy)| {
            if !strategy.enabled {
                return false;
            }
            if !strategy.selector.matches(entity) {
                return false;
            }
            for condition in &strategy.conditions {
                if let RuleCondition::Labels { labels } = condition
                    && !labels
                        .iter()
                        .all(|label| document_labels.contains(&label.as_str()))
                {
                    return false;
                }
            }
            true
        })
        .map(|&(id, s)| (id, s))
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

    fn default_spec() -> Strategy {
        Strategy::Text(TextStrategy::Mask { mask_char: '*' })
    }

    #[test]
    fn skips_below_threshold() {
        let entities: Entities = vec![test_entity("John", 0.5)].into();
        let entries = evaluate(&entities, &[], &default_spec(), 0.8, &[]);
        assert!(entries.is_empty());
    }

    #[test]
    fn produces_entry_above_threshold() {
        let entities: Entities = vec![test_entity("John", 0.9)].into();
        let entries = evaluate(&entities, &[], &default_spec(), 0.5, &[]);
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].redaction.is_applied);
    }

    #[test]
    fn uses_default_strategy_when_no_rules() {
        let spec = Strategy::Text(TextStrategy::Remove);
        let entities: Entities = vec![test_entity("secret", 0.9)].into();
        let entries = evaluate(&entities, &[], &spec, 0.0, &[]);
        assert_eq!(
            entries[0].redaction.strategy,
            Strategy::Text(TextStrategy::Remove)
        );
    }

    #[test]
    fn captures_original_value() {
        let entities: Entities = vec![test_entity("secret-value", 0.9)].into();
        let entries = evaluate(&entities, &[], &default_spec(), 0.0, &[]);
        assert_eq!(entries[0].value.original, "secret-value");
    }
}
