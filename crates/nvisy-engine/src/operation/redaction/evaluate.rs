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
use nvisy_ontology::policy::{Action, Condition, Strategy, StrategyPolicy};
use nvisy_ontology::provenance::{AuditEntry, RedactionMapping};
use nvisy_ontology::workflow::Redaction;
use uuid::Uuid;

use super::apply::RedactionApplicator;
use crate::operation::{Document, DocumentEnvelope, Operation};

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
        let (entries, mappings) = evaluate(
            &envelope.audit.entities,
            &strategies,
            &defaults,
            self.default_threshold,
            &document_labels,
            &envelope.document.metadata,
            &envelope.document,
        )
        .await;
        envelope.audit.entries.extend(entries);
        envelope.redaction_map.entries.extend(mappings);

        RedactionApplicator::new(envelope).apply().await?;

        Ok(())
    }
}

/// Evaluate strategy policies against entities, producing audit entries
/// and redaction mappings.
///
/// Each entity either matches a rule (in which case the rule's
/// [`Strategy`] is merged with `defaults` before storage) or falls
/// back to `defaults` outright once it clears the confidence
/// threshold. Every entry stores a complete `Strategy`; at apply time
/// the per-modality method is resolved via [`Strategy::for_location`].
async fn evaluate(
    entities: &Entities,
    strategies: &[(Uuid, &StrategyPolicy)],
    defaults: &Strategy,
    default_threshold: f64,
    document_labels: &[&str],
    metadata: &ContentMetadata,
    document: &Document,
) -> (Vec<AuditEntry>, Vec<RedactionMapping>) {
    let mut entries = Vec::new();
    let mut mappings = Vec::new();

    for entity in entities {
        let matched = find_matching_strategy(strategies, entity, document_labels, metadata);

        let (mut strategy, policy_id) = match matched {
            Some((policy_id, rule)) => match &rule.action {
                Action::Redact { strategy } => (strategy.clone(), Some(policy_id)),
                _ => {
                    tracing::debug!(
                        target: TARGET,
                        entity_id = %entity.id,
                        %policy_id,
                        action = ?rule.action,
                        "non-redact policy action",
                    );
                    continue;
                }
            },
            None => {
                if entity.confidence < default_threshold {
                    continue;
                }
                (Strategy::default(), None)
            }
        };

        // Fold defaults under the rule-level strategy: rule fields win,
        // unset fields fall back to the policy-set defaults, and any
        // still-unset modalities resolve to their per-modality Default
        // when the applicator calls Strategy::for_location.
        strategy.merge(defaults);

        let entity_id = entity.id;
        let original_value = document
            .value_at(&entity.location)
            .await
            .unwrap_or_else(|| format!("[{}]", entity.location));

        let mut builder = AuditEntry::builder().for_entity(
            entity_id,
            strategy,
            original_value.clone(),
            &entity.location,
        );
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
        mappings.push(RedactionMapping {
            entity_id,
            location: entity.location.clone(),
            original: original_value,
            replacement: None,
        });
    }

    tracing::info!(
        target: TARGET,
        entries = entries.len(),
        mappings = mappings.len(),
        "policy evaluation complete",
    );

    (entries, mappings)
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
    use nvisy_ontology::entity::Entity;
    use nvisy_ontology::policy::TextStrategy;

    use super::*;
    use crate::operation::Document;

    fn test_entity(value: &str, confidence: f64) -> Entity {
        Entity::test_builder(0, value.len())
            .with_confidence(confidence)
            .test_build()
    }

    fn defaults() -> Strategy {
        Strategy::text(TextStrategy::Mask { mask_char: '*' })
    }

    #[tokio::test]
    async fn skips_below_threshold() {
        let doc = Document::from_text("John").await;
        let entities: Entities = vec![test_entity("John", 0.5)].into();
        let (entries, _mappings) = evaluate(
            &entities,
            &[],
            &defaults(),
            0.8,
            &[],
            &ContentMetadata::new(),
            &doc,
        )
        .await;
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn produces_entry_above_threshold() {
        let doc = Document::from_text("John").await;
        let entities: Entities = vec![test_entity("John", 0.9)].into();
        let (entries, _mappings) = evaluate(
            &entities,
            &[],
            &defaults(),
            0.5,
            &[],
            &ContentMetadata::new(),
            &doc,
        )
        .await;
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].redaction.is_applied);
    }

    #[tokio::test]
    async fn uses_default_strategy_when_no_rules() {
        let doc = Document::from_text("secret").await;
        let defaults = Strategy::text(TextStrategy::Remove);
        let entities: Entities = vec![test_entity("secret", 0.9)].into();
        let (entries, _mappings) = evaluate(
            &entities,
            &[],
            &defaults,
            0.0,
            &[],
            &ContentMetadata::new(),
            &doc,
        )
        .await;
        assert_eq!(
            entries[0].redaction.strategy.text.as_ref(),
            Some(&TextStrategy::Remove),
        );
    }

    #[tokio::test]
    async fn captures_original_value() {
        let doc = Document::from_text("secret-value").await;
        let entities: Entities = vec![test_entity("secret-value", 0.9)].into();
        let (entries, _mappings) = evaluate(
            &entities,
            &[],
            &defaults(),
            0.0,
            &[],
            &ContentMetadata::new(),
            &doc,
        )
        .await;
        assert_eq!(entries[0].value.original, "secret-value");
    }
}
