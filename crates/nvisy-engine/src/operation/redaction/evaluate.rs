//! Redaction operation.
//!
//! Runs at **phase 4** alongside [`GenerateContextOp`]. Evaluates policy
//! rules against detected entities to produce redaction records, then
//! builds and applies redaction instructions across all modalities
//! (text, image, audio) via [`RedactionApplicator`].
//!
//! [`GenerateContextOp`]: crate::operation::GenerateContextOp

use nvisy_core::Result;
use nvisy_core::content::ContentMetadata;
use nvisy_ontology::entity::{Entities, Entity};
use nvisy_ontology::policy::{Action, Condition, DefaultStrategy, StrategyPolicy};
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
        let defaults = policies.default_strategy();

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
            &defaults,
            self.default_threshold,
            &document_labels,
            &envelope.metadata,
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
    defaults: &DefaultStrategy,
    default_threshold: f64,
    document_labels: &[&str],
    metadata: &ContentMetadata,
) -> Vec<AuditEntry> {
    let mut entries = Vec::new();

    for entity in entities {
        let matched = find_matching_strategy(strategies, entity, document_labels, metadata);

        let (spec, policy_id) = match matched {
            Some((policy_id, strategy)) => match &strategy.action {
                Action::Redact { strategy } => (strategy.clone(), Some(policy_id)),
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
                let Some(spec) = defaults.for_location(&entity.location) else {
                    continue;
                };
                (spec, None)
            }
        };

        let entity_id = entity.id;
        let original_value = entity
            .text_value()
            .map(String::from)
            .unwrap_or_else(|| format!("[{}]", entity.location));
        let mut builder = AuditEntry::builder().for_entity(entity_id, spec, original_value);
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
    metadata: &ContentMetadata,
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
                if !condition.matches(document_labels, metadata) {
                    return false;
                }
            }
            true
        })
        .map(|&(id, s)| (id, s))
}

/// Extension trait for evaluating [`Condition`]s against document context.
trait ConditionExt {
    /// Returns `true` if this condition is satisfied by the given context.
    fn matches(&self, document_labels: &[&str], metadata: &ContentMetadata) -> bool;
}

impl ConditionExt for Condition {
    fn matches(&self, document_labels: &[&str], metadata: &ContentMetadata) -> bool {
        match self {
            Condition::Labels { labels } => labels.iter().all(|label| {
                document_labels
                    .iter()
                    .any(|doc| doc.eq_ignore_ascii_case(label))
            }),
            Condition::Metadata { key, value } => match metadata.get_extra(key) {
                Some(actual) => match value {
                    Some(expected) => actual.as_str().is_some_and(|s| s == expected),
                    None => true,
                },
                None => false,
            },
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{Entity, EntityCategory, EntityKind, Location, RecognitionMethod};
    use nvisy_ontology::policy::{Strategy, TextStrategy};

    use super::*;

    fn test_entity(value: &str, confidence: f64) -> Entity {
        use nvisy_ontology::entity::TextLocation;

        Entity::builder()
            .with_category(EntityCategory::PersonalIdentity)
            .with_entity_kind(EntityKind::PersonName)
            .with_recognition_methods(vec![RecognitionMethod::regex("test")])
            .with_confidence(confidence)
            .with_location(Location::from(
                TextLocation::builder()
                    .with_value(value)
                    .with_start_offset(0usize)
                    .with_end_offset(value.len())
                    .with_span_index(0usize)
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap()
    }

    fn defaults() -> DefaultStrategy {
        DefaultStrategy {
            text: Some(TextStrategy::Mask { mask_char: '*' }),
            ..Default::default()
        }
    }

    #[test]
    fn skips_below_threshold() {
        let entities: Entities = vec![test_entity("John", 0.5)].into();
        let entries = evaluate(
            &entities,
            &[],
            &defaults(),
            0.8,
            &[],
            &ContentMetadata::new(),
        );
        assert!(entries.is_empty());
    }

    #[test]
    fn produces_entry_above_threshold() {
        let entities: Entities = vec![test_entity("John", 0.9)].into();
        let entries = evaluate(
            &entities,
            &[],
            &defaults(),
            0.5,
            &[],
            &ContentMetadata::new(),
        );
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].redaction.is_applied);
    }

    #[test]
    fn uses_default_strategy_when_no_rules() {
        let defaults = DefaultStrategy {
            text: Some(TextStrategy::Remove),
            ..Default::default()
        };
        let entities: Entities = vec![test_entity("secret", 0.9)].into();
        let entries = evaluate(&entities, &[], &defaults, 0.0, &[], &ContentMetadata::new());
        assert_eq!(
            entries[0].redaction.strategy,
            Strategy::Text(TextStrategy::Remove)
        );
    }

    #[test]
    fn skips_entity_without_matching_modality_default() {
        let defaults = DefaultStrategy {
            image: Some(nvisy_ontology::policy::ImageStrategy::Blur { sigma: 15.0 }),
            ..Default::default()
        };
        let entities: Entities = vec![test_entity("text-entity", 0.9)].into();
        let entries = evaluate(&entities, &[], &defaults, 0.0, &[], &ContentMetadata::new());
        assert!(entries.is_empty());
    }

    #[test]
    fn captures_original_value() {
        let entities: Entities = vec![test_entity("secret-value", 0.9)].into();
        let entries = evaluate(
            &entities,
            &[],
            &defaults(),
            0.0,
            &[],
            &ContentMetadata::new(),
        );
        assert_eq!(entries[0].value.original, "secret-value");
    }
}
