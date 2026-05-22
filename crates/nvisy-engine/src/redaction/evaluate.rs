//! Redaction operation.
//!
//! Runs at **phase 4** alongside [`GenerateContext`]. Evaluates policy
//! rules against detected entities to produce redaction records, then
//! builds and applies redaction instructions across all modalities
//! (text, image, audio) via [`RedactionApplicator`].
//!
//! [`GenerateContext`]: GenerateContext (removed)

use nvisy_core::Result;
use nvisy_core::content::ContentMetadata;
use nvisy_ontology::entity::{Entities, Entity};
use nvisy_ontology::policy::{Action, Condition, Strategy, StrategyPolicy};
use nvisy_ontology::provenance::{AuditEntry, AuditEntryStatus, RedactionMapping};
use uuid::Uuid;

use super::apply::RedactionApplicator;
use super::defaults::RedactionDefaults;
use crate::envelope::{Document, DocumentEnvelope};
use crate::redaction::Redaction as RedactionConfig;

const TARGET: &str = "nvisy_engine::redaction";

/// Redaction operation: evaluates policies and applies redaction instructions.
pub struct Redactor {
    default_threshold: f64,
    process_metadata: bool,
}

impl Redactor {
    /// Build from workflow config + server-wide defaults.
    ///
    /// Each workflow field falls back to the matching
    /// [`RedactionDefaults`] value when unset.
    pub fn new(cfg: &RedactionConfig, defaults: &RedactionDefaults) -> Self {
        Self {
            default_threshold: cfg
                .confidence_threshold
                .unwrap_or(defaults.confidence_threshold),
            process_metadata: cfg.process_metadata.unwrap_or(defaults.process_metadata),
        }
    }

    /// `true` when metadata stripping is enabled.
    #[must_use]
    pub fn process_metadata(&self) -> bool {
        self.process_metadata
    }

    /// Evaluate policies and apply redaction instructions to the envelope.
    pub async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
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
        // Collect every strategy that matches this entity (selector +
        // conditions + enabled). `strategies` is already sorted by rank
        // (`StrategyPolicy::priority` ascending, then policy
        // precedence, then insertion order — see
        // [`Policies::all_strategies`]), so the first matching Redact
        // and the first matching Suppress are each best in rank order.
        let matching = matching_strategies(strategies, entity, document_labels, metadata);
        let best_redact_idx = matching
            .iter()
            .position(|(_, rule)| matches!(rule.action, Action::Redact { .. }));
        let best_suppress_idx = matching
            .iter()
            .position(|(_, rule)| matches!(rule.action, Action::Suppress));

        // Suppress wins when its priority is at least as high (lower
        // numeric value) as the best matching Redact's, or when no
        // Redact matches at all. We compare `StrategyPolicy::priority`
        // directly rather than slice index, so ties go to Suppress
        // regardless of the order the strategies were inserted.
        let suppress_wins = match (best_suppress_idx, best_redact_idx) {
            (Some(s), Some(r)) => matching[s].1.priority() <= matching[r].1.priority(),
            (Some(_), None) => true,
            _ => false,
        };

        if suppress_wins {
            let (policy_id, _) = matching[best_suppress_idx.expect("suppress_wins implies Some")];
            let entity_id = entity.id;
            let original_value = document
                .value_at(&entity.location)
                .await
                .unwrap_or_else(|| format!("[{}]", entity.location));

            let entry = AuditEntry::builder()
                .for_entity(
                    entity_id,
                    Strategy::default(),
                    original_value.clone(),
                    &entity.location,
                )
                .with_policy_id(policy_id)
                .with_status(AuditEntryStatus::Suppressed)
                .build()
                .expect("all required fields set");

            tracing::debug!(
                target: TARGET,
                %entity_id,
                %policy_id,
                "entity suppressed by policy",
            );

            entries.push(entry);
            // No `redaction_map` entry: nothing to apply, nothing to
            // map. The audit entry is the sole record.
            continue;
        }

        let (mut strategy, policy_id) = match best_redact_idx {
            Some(idx) => {
                let (policy_id, rule) = matching[idx];
                match &rule.action {
                    Action::Redact { strategy } => (strategy.clone(), Some(policy_id)),
                    _ => unreachable!("best_redact_idx is filtered to Action::Redact"),
                }
            }
            None => {
                // No matching Redact and no winning Suppress. Other
                // matched actions (Review/Alert/Block) currently fall
                // through to the default path; they get their own
                // dedicated handling in a follow-up.
                if !matching.is_empty() {
                    tracing::debug!(
                        target: TARGET,
                        entity_id = %entity.id,
                        actions = ?matching.iter().map(|(_, r)| &r.action).collect::<Vec<_>>(),
                        "matched non-redact, non-suppress action; falling through to default",
                    );
                }
                if entity.confidence.get() < default_threshold {
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

/// All strategies that apply to `entity`, in rank order (best first).
///
/// `strategies` is assumed to already be sorted by rank
/// (`StrategyPolicy::priority` ascending, then policy precedence, then
/// insertion order — see [`Policies::all_strategies`]); this function
/// filters by enabled + selector + conditions but preserves the input
/// ordering.
///
/// [`Policies::all_strategies`]: nvisy_ontology::policy::Policies::all_strategies
fn matching_strategies<'a>(
    strategies: &[(Uuid, &'a StrategyPolicy)],
    entity: &Entity,
    document_labels: &[&str],
    metadata: &ContentMetadata,
) -> Vec<(Uuid, &'a StrategyPolicy)> {
    strategies
        .iter()
        .filter(|(_, strategy)| {
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
        .collect()
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
    use nvisy_ontology::policy::{EntitySelector, TextStrategy};
    use nvisy_ontology::primitive::Confidence;

    use super::*;
    use crate::envelope::Document;

    fn test_entity(value: &str, confidence: f64) -> Entity {
        Entity::test_builder(0, value.len())
            .with_confidence(Confidence::new(confidence).expect("in range"))
            .test_build()
    }

    fn defaults() -> Strategy {
        Strategy::text(TextStrategy::Mask { mask_char: '*' })
    }

    fn rule(action: Action, priority: Option<i32>) -> StrategyPolicy {
        StrategyPolicy {
            selector: EntitySelector::all(),
            action,
            priority,
            conditions: Vec::new(),
            enabled: true,
        }
    }

    fn redact_rule(priority: Option<i32>) -> StrategyPolicy {
        rule(
            Action::Redact {
                strategy: Strategy::text(TextStrategy::Remove),
            },
            priority,
        )
    }

    fn suppress_rule(priority: Option<i32>) -> StrategyPolicy {
        rule(Action::Suppress, priority)
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

    #[tokio::test]
    async fn suppress_only_emits_suppressed_entry_no_mapping() {
        let doc = Document::from_text("john").await;
        let entities: Entities = vec![test_entity("john", 0.9)].into();
        let policy_id = Uuid::now_v7();
        let suppress = suppress_rule(Some(0));
        let strategies = vec![(policy_id, &suppress)];

        let (entries, mappings) = evaluate(
            &entities,
            &strategies,
            &defaults(),
            0.5,
            &[],
            &ContentMetadata::new(),
            &doc,
        )
        .await;

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, AuditEntryStatus::Suppressed);
        assert_eq!(entries[0].policy_id, Some(policy_id));
        assert_eq!(entries[0].value.original, "john");
        assert!(entries[0].value.replacement.is_none());
        assert!(mappings.is_empty(), "no redaction_map entry for suppressed");
    }

    #[tokio::test]
    async fn suppress_at_higher_priority_beats_redact() {
        // Suppress at priority 0 (best) vs Redact at priority 10.
        let doc = Document::from_text("john").await;
        let entities: Entities = vec![test_entity("john", 0.9)].into();
        let suppress = suppress_rule(Some(0));
        let redact = redact_rule(Some(10));
        let strategies = vec![(Uuid::now_v7(), &suppress), (Uuid::now_v7(), &redact)];
        // Sort by priority so the rank order matches Policies::all_strategies.
        let mut sorted = strategies.clone();
        sorted.sort_by_key(|(_, s)| s.priority());

        let (entries, mappings) = evaluate(
            &entities,
            &sorted,
            &defaults(),
            0.5,
            &[],
            &ContentMetadata::new(),
            &doc,
        )
        .await;

        assert_eq!(entries[0].status, AuditEntryStatus::Suppressed);
        assert!(mappings.is_empty());
    }

    #[tokio::test]
    async fn suppress_at_equal_priority_wins_tiebreak() {
        // Suppress and Redact both at priority 0 — Suppress wins by the
        // "same or higher" rule.
        let doc = Document::from_text("john").await;
        let entities: Entities = vec![test_entity("john", 0.9)].into();
        let suppress = suppress_rule(Some(0));
        let redact = redact_rule(Some(0));
        // Suppress listed second to prove rank, not insertion, decides.
        let strategies = vec![(Uuid::now_v7(), &redact), (Uuid::now_v7(), &suppress)];

        let (entries, _mappings) = evaluate(
            &entities,
            &strategies,
            &defaults(),
            0.5,
            &[],
            &ContentMetadata::new(),
            &doc,
        )
        .await;

        assert_eq!(entries[0].status, AuditEntryStatus::Suppressed);
    }

    #[tokio::test]
    async fn redact_at_higher_priority_beats_suppress() {
        // Redact at priority 0 (best) vs Suppress at priority 10:
        // Suppress's rank > Redact's, so Redact wins.
        let doc = Document::from_text("john").await;
        let entities: Entities = vec![test_entity("john", 0.9)].into();
        let redact = redact_rule(Some(0));
        let suppress = suppress_rule(Some(10));
        let mut strategies = vec![(Uuid::now_v7(), &redact), (Uuid::now_v7(), &suppress)];
        strategies.sort_by_key(|(_, s)| s.priority());

        let (entries, mappings) = evaluate(
            &entities,
            &strategies,
            &defaults(),
            0.5,
            &[],
            &ContentMetadata::new(),
            &doc,
        )
        .await;

        assert_ne!(entries[0].status, AuditEntryStatus::Suppressed);
        assert_eq!(mappings.len(), 1);
    }
}
