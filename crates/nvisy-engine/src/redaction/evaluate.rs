//! Policy evaluation: walks the envelope's detected entities, picks
//! the winning per-entity action via [`PolicyStore::resolve`], and
//! writes typed [`AuditEntry<M>`]s onto each [`EntityRecord<M>`]. The
//! codec apply pass runs separately through
//! [`ApplyRedactions::apply_pending`].
//!
//! The chain-walk algorithm itself lives on [`PolicyStore::resolve`];
//! this module is the per-entity orchestration that wraps each
//! [`Decision`] into an audit entry.
//!
//! [`EntityRecord<M>`]: nvisy_ontology::provenance::EntityRecord
//! [`Decision`]: crate::envelope::Decision

use nvisy_core::Result;
use nvisy_core::content::ContentMetadata;
use nvisy_ontology::modality::{Modality, Text};
use nvisy_ontology::primitive::ConfidenceThreshold;
use nvisy_ontology::provenance::{
    AuditEntry, Decision as AuditDecision, EntityRecord, EntryMetadata, Execution,
    TabularReplacement, TextReplacement,
};

use super::apply;
use super::defaults::RedactionDefaults;
#[cfg(feature = "audio")]
use super::strategy::to_audio_redaction;
#[cfg(feature = "image")]
use super::strategy::to_image_redaction;
use super::strategy::{to_tabular_redaction, to_text_redaction};
use crate::envelope::value_at::ValueAt;
use crate::envelope::{Decision, DocumentEnvelope};
use crate::redaction::Redaction as RedactionConfig;

const TARGET: &str = "nvisy_engine::redaction";

/// Redaction operation: evaluates policies and applies redaction
/// instructions to a modality-typed envelope.
pub struct Redactor {
    default_threshold: ConfidenceThreshold,
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
    pub fn default_threshold(&self) -> ConfidenceThreshold {
        self.default_threshold
    }

    /// Evaluate policies, attach an [`AuditEntry<M>`] to each
    /// [`EntityRecord<M>`] the policy chain decides on, then hand
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

        let metadata = envelope.metadata.clone();
        let document_labels: Vec<&str> = Vec::new();

        let mut records = std::mem::take(&mut envelope.document.audit.records);
        evaluate::<M>(
            &mut records,
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
                &view.entry.decision.strategy,
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
        apply::commit(
            self,
            assembled.applied,
            assembled.failed,
            |redaction| match redaction.output().replacement_value() {
                Some(value) => TextReplacement::Substituted {
                    value: value.to_owned(),
                },
                None => TextReplacement::Removed,
            },
        );
        Ok(())
    }
}

#[cfg(feature = "tabular")]
#[async_trait::async_trait]
impl ApplyRedactions for DocumentEnvelope<nvisy_ontology::modality::Tabular> {
    async fn apply_pending(&mut self) -> Result<()> {
        let assembled = apply::build(self, |view| {
            to_tabular_redaction(
                &view.entry.decision.strategy,
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
        apply::commit(
            self,
            assembled.applied,
            assembled.failed,
            |redaction| match redaction.output().replacement_value() {
                Some(value) => TabularReplacement::Substituted {
                    value: value.to_owned(),
                },
                // The codec doesn't surface DropColumn separately
                // through this path today; Substituted with empty
                // string covers Clear.
                None => TabularReplacement::Substituted {
                    value: String::new(),
                },
            },
        );
        Ok(())
    }
}

#[cfg(not(feature = "tabular"))]
#[async_trait::async_trait]
impl ApplyRedactions for DocumentEnvelope<nvisy_ontology::modality::Tabular> {
    async fn apply_pending(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "image")]
#[async_trait::async_trait]
impl ApplyRedactions for DocumentEnvelope<nvisy_ontology::modality::Image> {
    async fn apply_pending(&mut self) -> Result<()> {
        use nvisy_codec::handler::ImageOutput;
        use nvisy_ontology::policy::ImageMethodTag;
        let assembled = apply::build(self, |view| {
            to_image_redaction(&view.entry.decision.strategy)
        });
        if assembled.is_noop() {
            return Ok(());
        }
        if !assembled.batch.is_empty() {
            self.apply_image_redactions(assembled.batch).await?;
        }
        apply::commit(
            self,
            assembled.applied,
            assembled.failed,
            |redaction| match redaction.output() {
                ImageOutput::Blur { .. } => ImageMethodTag::Blur,
                ImageOutput::Block { .. } => ImageMethodTag::Block,
                ImageOutput::Pixelate { .. } => ImageMethodTag::Pixelate,
                // ImageOutput::Replace has no ImageStrategy producer
                // today (#225). When that variant becomes reachable,
                // extend ImageMethodTag accordingly.
                ImageOutput::Replace { .. } => ImageMethodTag::Block,
            },
        );
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
        use nvisy_codec::handler::AudioOutput;
        use nvisy_ontology::policy::AudioMethodTag;
        let assembled = apply::build(self, |view| {
            to_audio_redaction(&view.entry.decision.strategy)
        });
        if assembled.is_noop() {
            return Ok(());
        }
        if !assembled.batch.is_empty() {
            self.apply_audio_redactions(assembled.batch).await?;
        }
        apply::commit(
            self,
            assembled.applied,
            assembled.failed,
            |redaction| match redaction.output() {
                AudioOutput::Silence => AudioMethodTag::Silence,
                AudioOutput::Remove => AudioMethodTag::Remove,
                // AudioOutput::Replace has no AudioStrategy producer
                // today (#226). When that variant becomes reachable,
                // extend AudioMethodTag accordingly.
                AudioOutput::Replace { .. } => AudioMethodTag::Remove,
            },
        );
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

async fn evaluate<M>(
    records: &mut [EntityRecord<M>],
    default_threshold: ConfidenceThreshold,
    document_labels: &[&str],
    metadata: &ContentMetadata,
    envelope: &DocumentEnvelope<M>,
) where
    M: Modality,
    DocumentEnvelope<M>: ValueAt<M>,
{
    for record in records {
        let entity = &record.entity;
        let decision = envelope
            .shared
            .policies
            .resolve::<M>(entity, document_labels, metadata);

        match decision {
            Decision::Suppress { policy_id, rank } => {
                let detected_text = envelope
                    .value_at(&entity.location)
                    .await
                    .unwrap_or_default();
                record.audit = Some(audit_entry(
                    AuditDecision {
                        policy_id: Some(policy_id),
                        rank: Some(rank),
                        strategy: M::Strategy::default(),
                        detected_text,
                    },
                    Execution::Suppressed,
                ));
            }
            Decision::Redact {
                strategy,
                policy_id,
                rank,
            } => {
                let detected_text = envelope
                    .value_at(&entity.location)
                    .await
                    .unwrap_or_default();
                record.audit = Some(audit_entry(
                    AuditDecision {
                        policy_id: Some(policy_id),
                        rank: Some(rank),
                        strategy,
                        detected_text,
                    },
                    Execution::Pending,
                ));
            }
            Decision::Fallthrough => {
                if !default_threshold.admits(entity.confidence) {
                    continue;
                }
                let detected_text = envelope
                    .value_at(&entity.location)
                    .await
                    .unwrap_or_default();
                record.audit = Some(audit_entry(
                    AuditDecision {
                        policy_id: None,
                        rank: None,
                        strategy: M::Strategy::default(),
                        detected_text,
                    },
                    Execution::Pending,
                ));
            }
        }
    }
}

fn audit_entry<M: Modality>(decision: AuditDecision<M>, execution: Execution<M>) -> AuditEntry<M> {
    AuditEntry {
        decision,
        execution,
        metadata: EntryMetadata::now(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nvisy_core::content::ContentMetadata;
    use nvisy_ontology::entity::Entity;
    use nvisy_ontology::policy::{
        Action, EntitySelector, Policy, PolicyRule, RuleRank, TextStrategy,
    };
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

    fn redact_rule(strategy: TextStrategy) -> PolicyRule<Text> {
        PolicyRule {
            selector: EntitySelector::all(),
            action: Action::Redact { strategy },
            conditions: Vec::new(),
            enabled: true,
        }
    }

    fn suppress_rule() -> PolicyRule<Text> {
        PolicyRule {
            selector: EntitySelector::all(),
            action: Action::Suppress,
            conditions: Vec::new(),
            enabled: true,
        }
    }

    fn policy_with(rules: Vec<PolicyRule<Text>>) -> Arc<Policy<Text>> {
        Arc::new(Policy::<Text> {
            id: uuid::Uuid::now_v7(),
            name: "test".into(),
            version: Version::new(1, 0, 0),
            description: None,
            rules,
            default_strategy: None,
            retention: Vec::new(),
        })
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
            default_threshold: ConfidenceThreshold::clamped(0.8),
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
            default_threshold: ConfidenceThreshold::clamped(0.5),
            process_metadata: false,
        }
        .execute(&mut env)
        .await
        .expect("execute");
        assert_eq!(env.document.audit.entries().count(), 1);
        let entry = first_entry(&env);
        assert_eq!(entry.decision.detected_text, "secret");
        assert!(entry.decision.policy_id.is_none());
        assert!(entry.decision.rank.is_none());
    }

    #[tokio::test]
    async fn matching_redact_rule_wins() {
        let mut env = text_envelope("john").await;
        seed_records(&mut env, vec![ent(0, 4, 0.9)]);
        let policy = policy_with(vec![redact_rule(TextStrategy::Replace {
            placeholder: "X".into(),
        })]);
        let policy_id = policy.id;
        Arc::get_mut(&mut env.shared)
            .expect("unique Arc")
            .policies
            .set::<Text>(vec![policy]);

        Redactor {
            default_threshold: ConfidenceThreshold::clamped(0.5),
            process_metadata: false,
        }
        .execute(&mut env)
        .await
        .expect("execute");

        assert_eq!(env.document.audit.entries().count(), 1);
        let entry = first_entry(&env);
        assert_eq!(entry.decision.policy_id, Some(policy_id));
        assert_eq!(entry.decision.rank, Some(RuleRank::new(0, 0)));
        assert!(matches!(
            entry.decision.strategy,
            TextStrategy::Replace { .. }
        ));
    }

    #[tokio::test]
    async fn first_rule_wins_inside_policy() {
        let mut env = text_envelope("john").await;
        seed_records(&mut env, vec![ent(0, 4, 0.9)]);
        // suppress comes first, redact second — first wins.
        let policy = policy_with(vec![suppress_rule(), redact_rule(TextStrategy::Hash)]);
        Arc::get_mut(&mut env.shared)
            .expect("unique Arc")
            .policies
            .set::<Text>(vec![policy]);

        Redactor {
            default_threshold: ConfidenceThreshold::clamped(0.5),
            process_metadata: false,
        }
        .execute(&mut env)
        .await
        .expect("execute");

        assert_eq!(env.document.audit.entries().count(), 1);
        assert!(matches!(first_entry(&env).execution, Execution::Suppressed));
    }

    #[tokio::test]
    async fn higher_precedence_policy_wins_over_lower() {
        let mut env = text_envelope("john").await;
        seed_records(&mut env, vec![ent(0, 4, 0.9)]);
        // policy at index 0 (higher precedence) chooses Hash;
        // policy at index 1 chooses Replace. Hash wins.
        let p_high = policy_with(vec![redact_rule(TextStrategy::Hash)]);
        let p_low = policy_with(vec![redact_rule(TextStrategy::Replace {
            placeholder: "X".into(),
        })]);
        let high_id = p_high.id;
        Arc::get_mut(&mut env.shared)
            .expect("unique Arc")
            .policies
            .set::<Text>(vec![p_high, p_low]);

        Redactor {
            default_threshold: ConfidenceThreshold::clamped(0.5),
            process_metadata: false,
        }
        .execute(&mut env)
        .await
        .expect("execute");

        let entry = first_entry(&env);
        assert_eq!(entry.decision.policy_id, Some(high_id));
        assert_eq!(entry.decision.rank, Some(RuleRank::new(0, 0)));
        assert!(matches!(entry.decision.strategy, TextStrategy::Hash));
    }

    #[tokio::test]
    async fn policy_default_falls_through_to_next_policy() {
        // p_high has no matching rule but has a default; default
        // should fire BEFORE the chain moves to p_low's rules.
        let mut env = text_envelope("john").await;
        seed_records(&mut env, vec![ent(0, 4, 0.9)]);
        let p_high = Arc::new(Policy::<Text> {
            id: uuid::Uuid::now_v7(),
            name: "high".into(),
            version: Version::new(1, 0, 0),
            description: None,
            rules: vec![],
            default_strategy: Some(TextStrategy::Hash),
            retention: Vec::new(),
        });
        let p_low = policy_with(vec![redact_rule(TextStrategy::Replace {
            placeholder: "X".into(),
        })]);
        let high_id = p_high.id;
        Arc::get_mut(&mut env.shared)
            .expect("unique Arc")
            .policies
            .set::<Text>(vec![p_high, p_low]);

        Redactor {
            default_threshold: ConfidenceThreshold::clamped(0.5),
            process_metadata: false,
        }
        .execute(&mut env)
        .await
        .expect("execute");

        let entry = first_entry(&env);
        assert_eq!(entry.decision.policy_id, Some(high_id));
        assert_eq!(entry.decision.rank, Some(RuleRank::for_default(0)));
        assert!(matches!(entry.decision.strategy, TextStrategy::Hash));
    }
}
