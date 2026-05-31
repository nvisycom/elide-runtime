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
//! [`Decision`]: crate::core::Decision

#[cfg(feature = "audio")]
use nvisy_codec::handler::AudioOutput;
#[cfg(feature = "image")]
use nvisy_codec::handler::ImageOutput;
use nvisy_core::Result;
use nvisy_core::content::ContentMetadata;
use nvisy_ontology::modality::{Modality, Text};
#[cfg(feature = "audio")]
use nvisy_ontology::policy::AudioMethodTag;
#[cfg(feature = "image")]
use nvisy_ontology::policy::ImageMethodTag;
use nvisy_ontology::primitive::ConfidenceThreshold;
use nvisy_ontology::provenance::{
    AuditEntry, Decision as AuditDecision, EntityRecord, EntryMetadata, Execution,
    TabularReplacement, TextReplacement,
};

use super::apply;
#[cfg(feature = "audio")]
use super::strategy::to_audio_redaction;
#[cfg(feature = "image")]
use super::strategy::to_image_redaction;
use super::strategy::{to_tabular_redaction, to_text_redaction};
use crate::core::Decision;
use crate::pipeline::PhaseTarget;

pub(crate) const TARGET: &str = "nvisy_engine::redaction";

/// Per-modality applicator hook. Each modality envelope opts in via
/// a thin impl that converts its [`Strategy`] to the codec wire
/// type and forwards the assembled batch to the codec; the generic
/// `RedactionPhase::run` is parameterised over this so the apply
/// path is shared.
///
/// [`Strategy`]: nvisy_ontology::modality::Modality::Strategy
#[async_trait::async_trait]
pub trait ApplyRedactions<M: nvisy_ontology::modality::Modality>: Send + Sync {
    async fn apply_pending(target: &mut PhaseTarget<'_, M>) -> Result<()>;
}

/// Stateless marker hosting the per-modality [`ApplyRedactions`]
/// impl. Used as the dispatch type since `PhaseTarget<'_, M>` is
/// the per-call surface and isn't a sensible owner of trait impls.
pub struct ApplyRedactionsImpl;

#[async_trait::async_trait]
impl ApplyRedactions<Text> for ApplyRedactionsImpl {
    async fn apply_pending(target: &mut PhaseTarget<'_, Text>) -> Result<()> {
        let assembled = apply::build(target, |view| {
            to_text_redaction(
                &view.entry.decision.strategy,
                view.original,
                view.entity_kind,
            )
        })
        .await;
        if assembled.is_noop() {
            return Ok(());
        }
        if !assembled.batch.is_empty() {
            target
                .handle
                .lock()
                .await
                .apply_text_redactions(assembled.batch)
                .await?;
        }
        apply::commit(
            target,
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
impl ApplyRedactions<nvisy_ontology::modality::Tabular> for ApplyRedactionsImpl {
    async fn apply_pending(
        target: &mut PhaseTarget<'_, nvisy_ontology::modality::Tabular>,
    ) -> Result<()> {
        let assembled = apply::build(target, |view| {
            to_tabular_redaction(
                &view.entry.decision.strategy,
                view.original,
                view.entity_kind,
            )
        })
        .await;
        if assembled.is_noop() {
            return Ok(());
        }
        if !assembled.batch.is_empty() {
            target
                .handle
                .lock()
                .await
                .apply_tabular_redactions(assembled.batch)
                .await?;
        }
        apply::commit(
            target,
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
impl ApplyRedactions<nvisy_ontology::modality::Tabular> for ApplyRedactionsImpl {
    async fn apply_pending(
        _target: &mut PhaseTarget<'_, nvisy_ontology::modality::Tabular>,
    ) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "image")]
#[async_trait::async_trait]
impl ApplyRedactions<nvisy_ontology::modality::Image> for ApplyRedactionsImpl {
    async fn apply_pending(
        target: &mut PhaseTarget<'_, nvisy_ontology::modality::Image>,
    ) -> Result<()> {
        let assembled = apply::build(target, |view| {
            to_image_redaction(&view.entry.decision.strategy)
        })
        .await;
        if assembled.is_noop() {
            return Ok(());
        }
        if !assembled.batch.is_empty() {
            target
                .handle
                .lock()
                .await
                .apply_image_redactions(assembled.batch)
                .await?;
        }
        apply::commit(
            target,
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
impl ApplyRedactions<nvisy_ontology::modality::Image> for ApplyRedactionsImpl {
    async fn apply_pending(
        _target: &mut PhaseTarget<'_, nvisy_ontology::modality::Image>,
    ) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "audio")]
#[async_trait::async_trait]
impl ApplyRedactions<nvisy_ontology::modality::Audio> for ApplyRedactionsImpl {
    async fn apply_pending(
        target: &mut PhaseTarget<'_, nvisy_ontology::modality::Audio>,
    ) -> Result<()> {
        let assembled = apply::build(target, |view| {
            to_audio_redaction(&view.entry.decision.strategy)
        })
        .await;
        if assembled.is_noop() {
            return Ok(());
        }
        if !assembled.batch.is_empty() {
            target
                .handle
                .lock()
                .await
                .apply_audio_redactions(assembled.batch)
                .await?;
        }
        apply::commit(
            target,
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
impl ApplyRedactions<nvisy_ontology::modality::Audio> for ApplyRedactionsImpl {
    async fn apply_pending(
        _target: &mut PhaseTarget<'_, nvisy_ontology::modality::Audio>,
    ) -> Result<()> {
        Ok(())
    }
}

pub(crate) async fn evaluate<M>(
    records: &mut [EntityRecord<M>],
    default_threshold: ConfidenceThreshold,
    document_labels: &[&str],
    metadata: &ContentMetadata,
    target: &PhaseTarget<'_, M>,
) where
    M: Modality,
{
    for record in records {
        let entity = &record.entity;
        let decision = target
            .shared
            .policies
            .resolve::<M>(entity, document_labels, metadata);

        match decision {
            Decision::Suppress { policy_id, rank } => {
                record.audit = Some(audit_entry(
                    AuditDecision {
                        policy_id: Some(policy_id),
                        rank: Some(rank),
                        strategy: M::Strategy::default(),
                    },
                    Execution::Suppressed,
                ));
            }
            Decision::Redact {
                strategy,
                policy_id,
                rank,
            } => {
                record.audit = Some(audit_entry(
                    AuditDecision {
                        policy_id: Some(policy_id),
                        rank: Some(rank),
                        strategy,
                    },
                    Execution::Pending,
                ));
            }
            Decision::Fallthrough => {
                if !default_threshold.admits(entity.confidence) {
                    continue;
                }
                record.audit = Some(audit_entry(
                    AuditDecision {
                        policy_id: None,
                        rank: None,
                        strategy: M::Strategy::default(),
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
    use nvisy_ontology::document::Document;
    use nvisy_ontology::entity::Entity;
    use nvisy_ontology::modality::{TextExtraction, TextMetadata};
    use nvisy_ontology::policy::{
        Action, EntitySelector, Policy, PolicyRule, RuleRank, TextStrategy,
    };
    use nvisy_ontology::primitive::Confidence;
    use semver::Version;
    use tokio::sync::Mutex;

    use super::super::run_redaction;
    use super::*;
    use crate::core::{SharedData, SharedHandle};

    /// Owned bundle that holds everything a test needs to build a
    /// fresh [`PhaseTarget<'_, Text>`] on demand. Tests build this
    /// once with `text_fixture(...)`, mutate `shared.policies` to
    /// install per-test policy, then create a target borrowing from
    /// the bundle to drive [`run_redaction`].
    struct Bundle {
        handle: SharedHandle,
        doc: Document<Text>,
        metadata: ContentMetadata,
        shared: Arc<SharedData>,
    }

    impl Bundle {
        fn target(&mut self) -> PhaseTarget<'_, Text> {
            PhaseTarget::<Text>::new(
                &mut self.doc,
                &self.handle,
                uuid::Uuid::nil(),
                &self.metadata,
                &self.shared,
            )
        }

        fn first_entry(&self) -> &AuditEntry<Text> {
            self.doc.audit.records[0]
                .audit
                .as_ref()
                .expect("first record has an audit entry")
        }
    }

    async fn text_fixture(text: &str) -> Bundle {
        let registry = crate::ingestion::registry::Registry::open(
            tempfile::tempdir().expect("tempdir").path(),
        )
        .expect("open registry");
        let shared = SharedData::new(uuid::Uuid::nil(), uuid::Uuid::nil(), registry);
        let handle: SharedHandle = Arc::new(Mutex::new(
            nvisy_formats::test_utils::decode_text(text)
                .await
                .expect("decode text"),
        ));
        let source = handle.lock().await.source();
        let doc = Document::<Text>::new(
            TextMetadata {
                extraction: TextExtraction::Native,
                languages: Vec::new(),
            },
            source,
        );
        Bundle {
            handle,
            doc,
            metadata: ContentMetadata::new().with_content_type("text/plain"),
            shared,
        }
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

    fn seed_records(bundle: &mut Bundle, entities: Vec<Entity<Text>>) {
        bundle.doc.audit.records = entities.into_iter().map(EntityRecord::new).collect();
    }

    fn install_policies(bundle: &mut Bundle, policies: Vec<Arc<Policy<Text>>>) {
        Arc::get_mut(&mut bundle.shared)
            .expect("unique Arc")
            .policies
            .set::<Text>(policies);
    }

    #[tokio::test]
    async fn skips_below_threshold_no_policies() {
        let mut bundle = text_fixture("john").await;
        seed_records(&mut bundle, vec![ent(0, 4, 0.4)]);
        run_redaction(ConfidenceThreshold::clamped(0.8), &mut bundle.target())
            .await
            .expect("execute");
        assert_eq!(bundle.doc.audit.entries().count(), 0);
    }

    #[tokio::test]
    async fn default_strategy_above_threshold() {
        let mut bundle = text_fixture("secret").await;
        seed_records(&mut bundle, vec![ent(0, 6, 0.9)]);
        run_redaction(ConfidenceThreshold::clamped(0.5), &mut bundle.target())
            .await
            .expect("execute");
        assert_eq!(bundle.doc.audit.entries().count(), 1);
        let entry = bundle.first_entry();
        assert!(entry.decision.policy_id.is_none());
        assert!(entry.decision.rank.is_none());
    }

    #[tokio::test]
    async fn matching_redact_rule_wins() {
        let mut bundle = text_fixture("john").await;
        seed_records(&mut bundle, vec![ent(0, 4, 0.9)]);
        let policy = policy_with(vec![redact_rule(TextStrategy::Replace {
            placeholder: "X".into(),
        })]);
        let policy_id = policy.id;
        install_policies(&mut bundle, vec![policy]);

        run_redaction(ConfidenceThreshold::clamped(0.5), &mut bundle.target())
            .await
            .expect("execute");

        assert_eq!(bundle.doc.audit.entries().count(), 1);
        let entry = bundle.first_entry();
        assert_eq!(entry.decision.policy_id, Some(policy_id));
        assert_eq!(entry.decision.rank, Some(RuleRank::new(0, 0)));
        assert!(matches!(
            entry.decision.strategy,
            TextStrategy::Replace { .. }
        ));
    }

    #[tokio::test]
    async fn first_rule_wins_inside_policy() {
        let mut bundle = text_fixture("john").await;
        seed_records(&mut bundle, vec![ent(0, 4, 0.9)]);
        // suppress comes first, redact second — first wins.
        let policy = policy_with(vec![suppress_rule(), redact_rule(TextStrategy::Hash)]);
        install_policies(&mut bundle, vec![policy]);

        run_redaction(ConfidenceThreshold::clamped(0.5), &mut bundle.target())
            .await
            .expect("execute");

        assert_eq!(bundle.doc.audit.entries().count(), 1);
        assert!(matches!(
            bundle.first_entry().execution,
            Execution::Suppressed
        ));
    }

    #[tokio::test]
    async fn higher_precedence_policy_wins_over_lower() {
        let mut bundle = text_fixture("john").await;
        seed_records(&mut bundle, vec![ent(0, 4, 0.9)]);
        // policy at index 0 (higher precedence) chooses Hash;
        // policy at index 1 chooses Replace. Hash wins.
        let p_high = policy_with(vec![redact_rule(TextStrategy::Hash)]);
        let p_low = policy_with(vec![redact_rule(TextStrategy::Replace {
            placeholder: "X".into(),
        })]);
        let high_id = p_high.id;
        install_policies(&mut bundle, vec![p_high, p_low]);

        run_redaction(ConfidenceThreshold::clamped(0.5), &mut bundle.target())
            .await
            .expect("execute");

        let entry = bundle.first_entry();
        assert_eq!(entry.decision.policy_id, Some(high_id));
        assert_eq!(entry.decision.rank, Some(RuleRank::new(0, 0)));
        assert!(matches!(entry.decision.strategy, TextStrategy::Hash));
    }

    #[tokio::test]
    async fn policy_default_falls_through_to_next_policy() {
        // p_high has no matching rule but has a default; default
        // should fire BEFORE the chain moves to p_low's rules.
        let mut bundle = text_fixture("john").await;
        seed_records(&mut bundle, vec![ent(0, 4, 0.9)]);
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
        install_policies(&mut bundle, vec![p_high, p_low]);

        run_redaction(ConfidenceThreshold::clamped(0.5), &mut bundle.target())
            .await
            .expect("execute");

        let entry = bundle.first_entry();
        assert_eq!(entry.decision.policy_id, Some(high_id));
        assert_eq!(entry.decision.rank, Some(RuleRank::for_default(0)));
        assert!(matches!(entry.decision.strategy, TextStrategy::Hash));
    }
}
