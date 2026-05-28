//! Policy evaluation: walks the envelope's detected entities, picks
//! the winning per-entity action, and records typed
//! [`AuditEntry<M>`]s and [`RedactionMapping<M>`]s. The codec apply
//! pass runs separately through [`ApplyRedactions::apply_pending`].

use nvisy_core::Result;
use nvisy_core::content::ContentMetadata;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::{Modality, Text};
use nvisy_ontology::policy::{Action, Condition, Policy, StrategyPolicy};
use nvisy_ontology::provenance::{AuditEntry, AuditEntryStatus, RedactionMapping};
use uuid::Uuid;

use super::apply::RedactionApplicator;
use super::defaults::RedactionDefaults;
use crate::envelope::DocumentEnvelope;
use crate::envelope::value_at::ValueAt;
use crate::redaction::Redaction as RedactionConfig;

const TARGET: &str = "nvisy_engine::redaction";

/// Redaction operation: evaluates policies and applies redaction
/// instructions to a modality-typed envelope.
pub struct Redactor {
    default_threshold: f64,
    process_metadata: bool,
}

impl Redactor {
    /// Build from workflow config + server-wide defaults.
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

    /// Threshold below which entities are skipped for redaction.
    #[must_use]
    pub fn default_threshold(&self) -> f64 {
        self.default_threshold
    }

    /// Evaluate policies, record audit entries + redaction mappings,
    /// then hand off to the codec applicator.
    pub async fn execute<M>(&self, envelope: &mut DocumentEnvelope<M>) -> Result<()>
    where
        M: Modality,
        DocumentEnvelope<M>: ValueAt<M> + ApplyRedactions,
    {
        if envelope.document.audit.entities.is_empty() {
            return Ok(());
        }

        // Borrow snapshots so we can `&mut envelope` later. `policies`
        // and `metadata` are cheap clones; entities are taken so the
        // borrow checker lets us populate `audit.entries` in the same
        // pass.
        let policies: Vec<Policy<M>> = envelope.shared.policies.get::<M>().to_vec();
        let metadata = envelope.metadata.clone();
        let strategies = rank_strategies(&policies);
        let document_labels: Vec<&str> = Vec::new();

        let entities = std::mem::take(&mut envelope.document.audit.entities);
        let (entries, mappings, kept_entities) = evaluate::<M>(
            entities,
            &strategies,
            self.default_threshold,
            &document_labels,
            &metadata,
            envelope,
        )
        .await;

        envelope.document.audit.entities = kept_entities;
        envelope.document.audit.entries.extend(entries);
        envelope.redaction_map.entries.extend(mappings);

        tracing::debug!(
            target: TARGET,
            entries = envelope.document.audit.entries.len(),
            mappings = envelope.redaction_map.entries.len(),
            "policy evaluation complete",
        );

        envelope.apply_pending().await?;
        Ok(())
    }
}

/// Per-modality applicator hook. Each modality envelope opts in via
/// a thin impl that calls the codec's typed redaction method; the
/// generic [`Redactor::execute`] above is parameterised over this so
/// the apply path is shared.
#[async_trait::async_trait]
pub trait ApplyRedactions {
    async fn apply_pending(&mut self) -> Result<()>;
}

#[async_trait::async_trait]
impl ApplyRedactions for DocumentEnvelope<Text> {
    async fn apply_pending(&mut self) -> Result<()> {
        RedactionApplicator::new(self).apply().await
    }
}

/// Flatten every policy's strategy list into a single (policy_id,
/// strategy) sequence sorted by `StrategyPolicy::priority` ascending.
/// Stable sort preserves per-policy declaration order within a
/// priority bucket.
fn rank_strategies<M: Modality>(policies: &[Policy<M>]) -> Vec<(Uuid, &StrategyPolicy<M>)> {
    let mut out: Vec<(Uuid, &StrategyPolicy<M>)> = policies
        .iter()
        .flat_map(|p| p.strategies.iter().map(move |s| (p.id, s)))
        .collect();
    out.sort_by_key(|(_, s)| s.priority());
    out
}

async fn evaluate<M>(
    entities: Vec<Entity<M>>,
    strategies: &[(Uuid, &StrategyPolicy<M>)],
    default_threshold: f64,
    document_labels: &[&str],
    metadata: &ContentMetadata,
    envelope: &DocumentEnvelope<M>,
) -> (Vec<AuditEntry<M>>, Vec<RedactionMapping<M>>, Vec<Entity<M>>)
where
    M: Modality,
    DocumentEnvelope<M>: ValueAt<M>,
{
    let mut entries = Vec::new();
    let mut mappings = Vec::new();
    let mut kept = Vec::with_capacity(entities.len());

    for entity in entities {
        let matching = matching_strategies(strategies, &entity, document_labels, metadata);

        let best_redact_idx = matching
            .iter()
            .position(|(_, sp)| matches!(sp.action, Action::Redact { .. }));
        let best_suppress_idx = matching
            .iter()
            .position(|(_, sp)| matches!(sp.action, Action::Suppress));

        // Suppress wins ties (≤): explicit suppression trumps a
        // same-priority redaction.
        let suppress_wins = match (best_suppress_idx, best_redact_idx) {
            (Some(s), Some(r)) => matching[s].1.priority() <= matching[r].1.priority(),
            (Some(_), None) => true,
            _ => false,
        };

        if suppress_wins {
            let (policy_id, _) = matching[best_suppress_idx.expect("checked above")];
            let original = envelope
                .value_at(&entity.location)
                .await
                .unwrap_or_default();
            let entry = AuditEntry::<M>::builder()
                .for_entity(entity.id, M::Strategy::default(), original)
                .with_policy_id(policy_id)
                .with_status(AuditEntryStatus::Suppressed)
                .build()
                .expect("audit entry fields set");
            entries.push(entry);
            // Suppressed entities still survive on audit.entities so
            // they appear in the compliance trail; the entry's status
            // marks the outcome.
            kept.push(entity);
            continue;
        }

        let (strategy, policy_id) = match best_redact_idx {
            Some(idx) => {
                let (policy_id, sp) = matching[idx];
                match &sp.action {
                    Action::Redact { strategy } => (strategy.clone(), Some(policy_id)),
                    _ => unreachable!("filtered to Action::Redact"),
                }
            }
            None => {
                // No matching Redact and no winning Suppress. Other
                // actions (Review/Alert/Block) currently fall through
                // to the default path; they get their own dedicated
                // handling in a follow-up.
                if entity.confidence.get() < default_threshold {
                    kept.push(entity);
                    continue;
                }
                (M::Strategy::default(), None)
            }
        };

        let original = envelope
            .value_at(&entity.location)
            .await
            .unwrap_or_default();
        let mut builder = AuditEntry::<M>::builder().for_entity(entity.id, strategy, original);
        if let Some(id) = policy_id {
            builder = builder.with_policy_id(id);
        }
        let entry = builder.build().expect("audit entry fields set");
        entries.push(entry);

        mappings.push(RedactionMapping {
            entity_id: entity.id,
            location: entity.location.clone(),
        });
        kept.push(entity);
    }

    (entries, mappings, kept)
}

/// Rank-ordered, selector-and-condition-matching strategies for one
/// entity. Input `strategies` is already rank-sorted (see
/// [`rank_strategies`]); this filter preserves that order.
fn matching_strategies<'a, M: Modality>(
    strategies: &[(Uuid, &'a StrategyPolicy<M>)],
    entity: &Entity<M>,
    document_labels: &[&str],
    metadata: &ContentMetadata,
) -> Vec<(Uuid, &'a StrategyPolicy<M>)> {
    strategies
        .iter()
        .filter(|(_, sp)| {
            sp.enabled
                && sp.selector.matches(entity)
                && sp
                    .conditions
                    .iter()
                    .all(|c| condition_matches(c, document_labels, metadata))
        })
        .map(|&(id, sp)| (id, sp))
        .collect()
}

fn condition_matches(
    condition: &Condition,
    document_labels: &[&str],
    metadata: &ContentMetadata,
) -> bool {
    match condition {
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nvisy_core::content::ContentMetadata;
    use nvisy_ontology::entity::Entity;
    use nvisy_ontology::policy::{Action, EntitySelector, StrategyPolicy, TextStrategy};
    use nvisy_ontology::primitive::Confidence;
    use semver::Version;
    use tokio::sync::Mutex;

    use super::*;
    use crate::envelope::SharedData;

    async fn text_envelope(text: &str) -> DocumentEnvelope<Text> {
        let registry = crate::ingestion::registry::Registry::open(
            tempfile::tempdir().expect("tempdir").path(),
        )
        .expect("open registry");
        let shared = SharedData::new(uuid::Uuid::nil(), uuid::Uuid::nil(), registry);
        let handle = nvisy_formats::test_utils::decode_text(text)
            .await
            .expect("decode text");
        DocumentEnvelope::<Text>::new(
            Arc::new(Mutex::new(handle)),
            ContentMetadata::new().with_content_type("text/plain"),
            shared,
        )
        .await
    }

    fn ent(start: usize, end: usize, conf: f64) -> Entity<Text> {
        Entity::test_builder(start, end)
            .with_confidence(Confidence::new(conf).expect("[0,1]"))
            .test_build()
    }

    fn redact_rule(priority: Option<i32>, strategy: TextStrategy) -> StrategyPolicy<Text> {
        StrategyPolicy {
            selector: EntitySelector::all(),
            action: Action::Redact { strategy },
            priority,
            conditions: Vec::new(),
            enabled: true,
        }
    }

    fn suppress_rule(priority: Option<i32>) -> StrategyPolicy<Text> {
        StrategyPolicy {
            selector: EntitySelector::all(),
            action: Action::Suppress,
            priority,
            conditions: Vec::new(),
            enabled: true,
        }
    }

    fn policy_with(strategies: Vec<StrategyPolicy<Text>>) -> Policy<Text> {
        Policy::<Text> {
            id: uuid::Uuid::now_v7(),
            name: "test".into(),
            version: Version::new(1, 0, 0),
            description: None,
            default_strategy: None,
            strategies,
            retention: Vec::new(),
        }
    }

    #[tokio::test]
    async fn skips_below_threshold_no_policies() {
        let mut env = text_envelope("john").await;
        env.document.audit.entities = vec![ent(0, 4, 0.4)];
        Redactor {
            default_threshold: 0.8,
            process_metadata: false,
        }
        .execute(&mut env)
        .await
        .expect("execute");
        assert!(env.document.audit.entries.is_empty());
        assert!(env.redaction_map.entries.is_empty());
    }

    #[tokio::test]
    async fn default_strategy_above_threshold() {
        let mut env = text_envelope("secret").await;
        env.document.audit.entities = vec![ent(0, 6, 0.9)];
        Redactor {
            default_threshold: 0.5,
            process_metadata: false,
        }
        .execute(&mut env)
        .await
        .expect("execute");
        assert_eq!(env.document.audit.entries.len(), 1);
        assert_eq!(env.redaction_map.entries.len(), 1);
        assert_eq!(env.document.audit.entries[0].value.original, "secret");
        assert!(env.document.audit.entries[0].policy_id.is_none());
    }

    #[tokio::test]
    async fn matching_redact_rule_wins() {
        let mut env = text_envelope("john").await;
        env.document.audit.entities = vec![ent(0, 4, 0.9)];
        let policy = policy_with(vec![redact_rule(
            Some(0),
            TextStrategy::Replace {
                placeholder: "X".into(),
            },
        )]);
        let policy_id = policy.id;
        Arc::get_mut(&mut env.shared)
            .expect("unique Arc")
            .policies
            .set::<Text>(vec![policy]);

        Redactor {
            default_threshold: 0.5,
            process_metadata: false,
        }
        .execute(&mut env)
        .await
        .expect("execute");

        assert_eq!(env.document.audit.entries.len(), 1);
        assert_eq!(env.document.audit.entries[0].policy_id, Some(policy_id));
        assert!(matches!(
            env.document.audit.entries[0].redaction.strategy,
            TextStrategy::Replace { .. }
        ));
    }

    #[tokio::test]
    async fn suppress_beats_redact_at_equal_priority() {
        let mut env = text_envelope("john").await;
        env.document.audit.entities = vec![ent(0, 4, 0.9)];
        let policy = policy_with(vec![
            redact_rule(Some(0), TextStrategy::Hash),
            suppress_rule(Some(0)),
        ]);
        Arc::get_mut(&mut env.shared)
            .expect("unique Arc")
            .policies
            .set::<Text>(vec![policy]);

        Redactor {
            default_threshold: 0.5,
            process_metadata: false,
        }
        .execute(&mut env)
        .await
        .expect("execute");

        assert_eq!(env.document.audit.entries.len(), 1);
        assert_eq!(env.document.audit.entries[0].status, AuditEntryStatus::Suppressed);
        assert!(
            env.redaction_map.entries.is_empty(),
            "suppression records no mapping"
        );
    }

    #[tokio::test]
    async fn higher_priority_redact_wins_over_suppress() {
        let mut env = text_envelope("john").await;
        env.document.audit.entities = vec![ent(0, 4, 0.9)];
        let policy = policy_with(vec![
            redact_rule(Some(0), TextStrategy::Hash),
            suppress_rule(Some(10)),
        ]);
        Arc::get_mut(&mut env.shared)
            .expect("unique Arc")
            .policies
            .set::<Text>(vec![policy]);

        Redactor {
            default_threshold: 0.5,
            process_metadata: false,
        }
        .execute(&mut env)
        .await
        .expect("execute");

        assert_ne!(env.document.audit.entries[0].status, AuditEntryStatus::Suppressed);
        assert_eq!(env.redaction_map.entries.len(), 1);
    }
}
