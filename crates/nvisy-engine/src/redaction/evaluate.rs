//! Policy evaluation: walks the envelope's detected entities, picks
//! the winning per-entity action, and writes typed [`AuditEntry<M>`]s
//! onto each [`EntityRecord<M>`]. The codec apply pass runs separately
//! through [`ApplyRedactions::apply_pending`].
//!
//! [`EntityRecord<M>`]: nvisy_ontology::provenance::EntityRecord

use nvisy_core::Result;
use nvisy_core::content::ContentMetadata;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::{Modality, Text};
use nvisy_ontology::policy::{Action, Condition, Policy, StrategyPolicy};
use nvisy_ontology::provenance::{AuditEntry, AuditEntryStatus, EntityRecord};
use uuid::Uuid;

use super::apply;
use super::defaults::RedactionDefaults;
#[cfg(feature = "audio")]
use super::strategy::to_audio_redaction;
#[cfg(feature = "image")]
use super::strategy::to_image_redaction;
use super::strategy::{to_tabular_redaction, to_text_redaction};
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

    /// Evaluate policies, attach an [`AuditEntry<M>`] to each
    /// [`EntityRecord<M>`] the policy set decides on, then hand
    /// off to the codec applicator.
    ///
    /// [`EntityRecord<M>`]: nvisy_ontology::provenance::EntityRecord
    pub async fn execute<M>(&self, envelope: &mut DocumentEnvelope<M>) -> Result<()>
    where
        M: Modality,
        DocumentEnvelope<M>: ValueAt<M> + ApplyRedactions,
    {
        if envelope.document.audit.records.is_empty() {
            return Ok(());
        }

        // Snapshot inputs that are cheap to clone so we can keep an
        // immutable borrow of the envelope alive across the
        // per-entity `value_at` lookups while still mutating each
        // record's `audit` slot.
        let policies: Vec<Policy<M>> = envelope.shared.policies.get::<M>().to_vec();
        let metadata = envelope.metadata.clone();
        let strategies = rank_strategies(&policies);
        let document_labels: Vec<&str> = Vec::new();

        let mut records = std::mem::take(&mut envelope.document.audit.records);
        evaluate::<M>(
            &mut records,
            &strategies,
            self.default_threshold,
            &document_labels,
            &metadata,
            envelope,
        )
        .await;
        envelope.document.audit.records = records;

        tracing::debug!(
            target: TARGET,
            entries = envelope.document.audit.entries().count(),
            "policy evaluation complete",
        );

        envelope.apply_pending().await?;
        Ok(())
    }
}

/// Per-modality applicator hook. Each modality envelope opts in via
/// a thin impl that converts its [`Strategy`] to the codec wire
/// type and forwards the assembled batch to the codec; the generic
/// [`Redactor::execute`] above is parameterised over this so the
/// apply path is shared.
///
/// [`Strategy`]: nvisy_ontology::modality::Modality::Strategy
#[async_trait::async_trait]
pub trait ApplyRedactions {
    async fn apply_pending(&mut self) -> Result<()>;
}

#[async_trait::async_trait]
impl ApplyRedactions for DocumentEnvelope<Text> {
    async fn apply_pending(&mut self) -> Result<()> {
        let assembled = apply::build(self, |view| {
            to_text_redaction(
                &view.entry.redaction.strategy,
                view.original,
                view.entity_kind,
            )
        });
        if assembled.is_noop() {
            return Ok(());
        }
        if !assembled.batch.is_empty() {
            self.apply_text_redactions(assembled.batch).await?;
        }
        apply::commit(self, assembled.applied, assembled.failed, |entry, redaction| {
            entry.value.replacement =
                redaction.output().replacement_value().map(str::to_owned);
        });
        Ok(())
    }
}

#[cfg(feature = "tabular")]
#[async_trait::async_trait]
impl ApplyRedactions for DocumentEnvelope<nvisy_ontology::modality::Tabular> {
    async fn apply_pending(&mut self) -> Result<()> {
        let assembled = apply::build(self, |view| {
            to_tabular_redaction(
                &view.entry.redaction.strategy,
                view.original,
                view.entity_kind,
            )
        });
        if assembled.is_noop() {
            return Ok(());
        }
        if !assembled.batch.is_empty() {
            self.apply_tabular_redactions(assembled.batch).await?;
        }
        apply::commit(self, assembled.applied, assembled.failed, |entry, redaction| {
            entry.value.replacement =
                redaction.output().replacement_value().map(str::to_owned);
        });
        Ok(())
    }
}

#[cfg(not(feature = "tabular"))]
#[async_trait::async_trait]
impl ApplyRedactions for DocumentEnvelope<nvisy_ontology::modality::Tabular> {
    async fn apply_pending(&mut self) -> Result<()> {
        let _ = to_tabular_redaction;
        Ok(())
    }
}

#[cfg(feature = "image")]
#[async_trait::async_trait]
impl ApplyRedactions for DocumentEnvelope<nvisy_ontology::modality::Image> {
    async fn apply_pending(&mut self) -> Result<()> {
        let assembled =
            apply::build(self, |view| to_image_redaction(&view.entry.redaction.strategy));
        if assembled.is_noop() {
            return Ok(());
        }
        if !assembled.batch.is_empty() {
            self.apply_image_redactions(assembled.batch).await?;
        }
        // Image redactions don't produce a substitutable string;
        // the audit entry's `replacement` stays unset.
        apply::commit(self, assembled.applied, assembled.failed, |_entry, _redaction| {});
        Ok(())
    }
}

#[cfg(not(feature = "image"))]
#[async_trait::async_trait]
impl ApplyRedactions for DocumentEnvelope<nvisy_ontology::modality::Image> {
    async fn apply_pending(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "audio")]
#[async_trait::async_trait]
impl ApplyRedactions for DocumentEnvelope<nvisy_ontology::modality::Audio> {
    async fn apply_pending(&mut self) -> Result<()> {
        let assembled =
            apply::build(self, |view| to_audio_redaction(&view.entry.redaction.strategy));
        if assembled.is_noop() {
            return Ok(());
        }
        if !assembled.batch.is_empty() {
            self.apply_audio_redactions(assembled.batch).await?;
        }
        // Audio redactions don't produce a substitutable string;
        // the audit entry's `replacement` stays unset.
        apply::commit(self, assembled.applied, assembled.failed, |_entry, _redaction| {});
        Ok(())
    }
}

#[cfg(not(feature = "audio"))]
#[async_trait::async_trait]
impl ApplyRedactions for DocumentEnvelope<nvisy_ontology::modality::Audio> {
    async fn apply_pending(&mut self) -> Result<()> {
        Ok(())
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
    records: &mut [EntityRecord<M>],
    strategies: &[(Uuid, &StrategyPolicy<M>)],
    default_threshold: f64,
    document_labels: &[&str],
    metadata: &ContentMetadata,
    envelope: &DocumentEnvelope<M>,
) where
    M: Modality,
    DocumentEnvelope<M>: ValueAt<M>,
{
    for record in records {
        let entity = &record.entity;
        let matching = matching_strategies(strategies, entity, document_labels, metadata);

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
                .for_redaction(M::Strategy::default(), original)
                .with_policy_id(policy_id)
                .with_status(AuditEntryStatus::Suppressed)
                .build()
                .expect("audit entry fields set");
            record.audit = Some(entry);
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
                    continue;
                }
                (M::Strategy::default(), None)
            }
        };

        let original = envelope
            .value_at(&entity.location)
            .await
            .unwrap_or_default();
        let mut builder = AuditEntry::<M>::builder().for_redaction(strategy, original);
        if let Some(id) = policy_id {
            builder = builder.with_policy_id(id);
        }
        let entry = builder.build().expect("audit entry fields set");
        record.audit = Some(entry);
    }
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

    fn seed_records(env: &mut DocumentEnvelope<Text>, entities: Vec<Entity<Text>>) {
        env.document.audit.records = entities.into_iter().map(EntityRecord::new).collect();
    }

    fn first_entry(env: &DocumentEnvelope<Text>) -> &AuditEntry<Text> {
        env.document.audit.records[0]
            .audit
            .as_ref()
            .expect("first record has an audit entry")
    }

    #[tokio::test]
    async fn skips_below_threshold_no_policies() {
        let mut env = text_envelope("john").await;
        seed_records(&mut env, vec![ent(0, 4, 0.4)]);
        Redactor {
            default_threshold: 0.8,
            process_metadata: false,
        }
        .execute(&mut env)
        .await
        .expect("execute");
        assert_eq!(env.document.audit.entries().count(), 0);
    }

    #[tokio::test]
    async fn default_strategy_above_threshold() {
        let mut env = text_envelope("secret").await;
        seed_records(&mut env, vec![ent(0, 6, 0.9)]);
        Redactor {
            default_threshold: 0.5,
            process_metadata: false,
        }
        .execute(&mut env)
        .await
        .expect("execute");
        assert_eq!(env.document.audit.entries().count(), 1);
        let entry = first_entry(&env);
        assert_eq!(entry.value.original, "secret");
        assert!(entry.policy_id.is_none());
    }

    #[tokio::test]
    async fn matching_redact_rule_wins() {
        let mut env = text_envelope("john").await;
        seed_records(&mut env, vec![ent(0, 4, 0.9)]);
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

        assert_eq!(env.document.audit.entries().count(), 1);
        let entry = first_entry(&env);
        assert_eq!(entry.policy_id, Some(policy_id));
        assert!(matches!(
            entry.redaction.strategy,
            TextStrategy::Replace { .. }
        ));
    }

    #[tokio::test]
    async fn suppress_beats_redact_at_equal_priority() {
        let mut env = text_envelope("john").await;
        seed_records(&mut env, vec![ent(0, 4, 0.9)]);
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

        assert_eq!(env.document.audit.entries().count(), 1);
        assert_eq!(first_entry(&env).status, AuditEntryStatus::Suppressed);
    }

    #[tokio::test]
    async fn higher_priority_redact_wins_over_suppress() {
        let mut env = text_envelope("john").await;
        seed_records(&mut env, vec![ent(0, 4, 0.9)]);
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

        assert_ne!(first_entry(&env).status, AuditEntryStatus::Suppressed);
        assert_eq!(env.document.audit.entries().count(), 1);
    }
}
