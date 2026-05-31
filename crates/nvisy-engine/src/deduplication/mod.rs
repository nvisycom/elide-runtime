//! Entity deduplication phase.
//!
//! Runs after detection. Combines multiple detection passes into a
//! single, deduplicated set of entities with combined confidence
//! scores. Public surface is [`DeduplicationPhase`] +
//! [`DeduplicationParams`]; the sub-step traits live in private
//! submodules and are used only by the phase body.
//!
//! Pipeline order (inside `DeduplicationPhase::run`):
//!
//! 1. **Calibrate**: scale raw confidence scores using per-method
//!    multipliers from [`CalibrationMap`].
//! 2. **Filter**: drop entities below the per-run confidence
//!    threshold (and outside the plan's allowed-kinds list).
//! 3. **Group + fuse**: partition entities by kind, value, and
//!    location overlap (per [`GroupingCriteria`]), then combine each
//!    group into one entity using the [`DeduplicationStrategy`].
//! 4. **Conflict resolution**: resolve cross-kind span overlaps per
//!    the configured [`ConflictResolution`] strategy.

mod calibrate;
mod filter;
mod fuse;
mod params;
mod resolve;
mod span_size;

use std::marker::PhantomData;
use std::mem;

use nvisy_core::Result;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::{Modality, Overlap};
use nvisy_ontology::provenance::EntityRecord;

use self::calibrate::Calibrate;
pub use self::calibrate::CalibrationMap;
use self::filter::Filter;
pub use self::filter::FilterParams;
use self::fuse::Fuse;
pub use self::fuse::{DeduplicationStrategy, GroupingCriteria};
pub use self::params::DeduplicationParams;
pub use self::resolve::ConflictResolution;
use self::resolve::ResolveConflicts;
pub use self::span_size::SpanSize;
use crate::core::ValueAt;
use crate::pipeline::{ModalityKind, Phase, PhaseContext, PhaseInfo, PhaseTarget};

const TARGET: &str = "nvisy_engine::deduplication";

/// Deduplication phase: calibrate raw confidence scores, filter
/// below-threshold or wrong-kind entities, group + fuse overlapping
/// matches, then resolve cross-kind conflicts.
///
/// Stateless beyond the modality marker; per-run config
/// ([`DeduplicationParams`] for calibration/threshold/grouping,
/// [`Detection`] for the allowed-kinds list) comes from `ctx.plan`
/// each call.
///
/// [`Detection`]: crate::detection::Detection
pub struct DeduplicationPhase<M: Modality> {
    _marker: PhantomData<fn() -> M>,
}

impl<M: Modality> DeduplicationPhase<M> {
    /// Build the phase. Stateless beyond the modality marker.
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }

    /// Run the four-step pipeline against an owned entity list.
    /// Shared by [`Phase::run`] and the test harness so both go
    /// through the same code path.
    pub(crate) async fn deduplicate(
        params: &DeduplicationParams,
        filter: &FilterParams,
        mut entities: Vec<Entity<M>>,
        target: &PhaseTarget<'_, M>,
    ) -> Vec<Entity<M>>
    where
        M: Overlap + SpanSize,
        for<'a> PhaseTarget<'a, M>: ValueAt<M>,
    {
        if entities.is_empty() {
            return entities;
        }
        let before = entities.len();

        entities.calibrate(&params.calibration);
        let dropped = entities.filter(filter);
        entities
            .fuse(&params.strategy, params.grouping, target)
            .await;
        let _conflict_dropped = entities.resolve_conflicts(&params.conflict_resolution);

        tracing::info!(
            target: TARGET,
            before,
            after = entities.len(),
            reduced = before.saturating_sub(entities.len()),
            dropped = dropped.len(),
            "deduplication complete",
        );

        entities
    }
}

impl<M: Modality> Default for DeduplicationPhase<M> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl<M> Phase<M> for DeduplicationPhase<M>
where
    M: Modality + Overlap + SpanSize,
    for<'a> PhaseTarget<'a, M>: ValueAt<M>,
{
    fn inspect(&self) -> PhaseInfo {
        PhaseInfo {
            name: "deduplication",
            modality: ModalityKind::of::<M>(),
            mutating: true,
        }
    }

    async fn run(&self, ctx: &PhaseContext<'_, M>, target: &mut PhaseTarget<'_, M>) -> Result<()> {
        if target.doc.audit.records.is_empty() {
            return Ok(());
        }

        let detection = &ctx.plan.detection;
        let dedup = &ctx.plan.deduplication;
        let filter = FilterParams {
            allowed_kinds: (!detection.entity_kinds.is_empty())
                .then(|| detection.entity_kinds.clone()),
            confidence_threshold: dedup.confidence_threshold,
        };

        tracing::debug!(
            target: TARGET,
            entities = target.doc.audit.records.len(),
            "running deduplication",
        );

        // Dedup runs before redaction evaluation, so every record's
        // `audit` is still None; we can pull entities out, dedup, and
        // rewrap without losing audit state.
        let records = mem::take(&mut target.doc.audit.records);
        let entities: Vec<Entity<M>> = records.into_iter().map(|r| r.entity).collect();
        let deduped = Self::deduplicate(&ctx.plan.deduplication, &filter, entities, target).await;
        target.doc.audit.records = deduped.into_iter().map(EntityRecord::new).collect();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nvisy_core::content::ContentMetadata;
    use nvisy_ontology::document::Document;
    use nvisy_ontology::entity::{Entity, ModelKind, RecognitionMethod};
    use nvisy_ontology::modality::{Text, TextExtraction, TextMetadata};
    use nvisy_ontology::primitive::{Confidence, ConfidenceThreshold};
    use tokio::sync::Mutex;

    use super::*;
    use crate::core::{SharedData, SharedHandle};

    fn conf(v: f64) -> Confidence {
        Confidence::new(v).expect("confidence in [0,1]")
    }

    const TEST_TEXT: &str = "John Smith";

    /// Owned per-test components used to build a [`PhaseTarget`].
    struct Fixture {
        handle: SharedHandle,
        doc: Document<Text>,
        metadata: ContentMetadata,
        shared: Arc<SharedData>,
    }

    impl Fixture {
        fn target(&mut self) -> PhaseTarget<'_, Text> {
            PhaseTarget::<Text>::new(
                &mut self.doc,
                &self.handle,
                uuid::Uuid::nil(),
                &self.metadata,
                &self.shared,
            )
        }
    }

    async fn test_fixture(text: &str) -> Fixture {
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
        Fixture {
            handle,
            doc,
            metadata: ContentMetadata::new().with_content_type("text/plain"),
            shared,
        }
    }

    #[tokio::test]
    async fn confidence_threshold_filters() {
        let mut fix = test_fixture("John......Jane").await;
        let filter = FilterParams {
            confidence_threshold: Some(ConfidenceThreshold::clamped(0.85)),
            ..Default::default()
        };
        let entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 4).test_build(),
            Entity::test_builder(10, 14)
                .with_confidence(conf(0.5))
                .test_build(),
        ];
        let target = fix.target();
        let result = DeduplicationPhase::<Text>::deduplicate(
            &DeduplicationParams::default(),
            &filter,
            entities,
            &target,
        )
        .await;
        assert_eq!(result.len(), 1);
        let value = target.value_at(&result[0].location).await;
        assert_eq!(value.as_deref(), Some("John"));
    }

    #[tokio::test]
    async fn full_pipeline() {
        let mut fix = test_fixture(TEST_TEXT).await;
        let entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.7))
                .test_build(),
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.8))
                .test_build(),
            Entity::test_builder(0, 4)
                .with_recognition_methods(vec![RecognitionMethod::nlp_ner(
                    "test",
                    ModelKind::SelfHosted,
                )])
                .with_confidence(conf(0.85))
                .test_build(),
        ];
        let target = fix.target();
        let result = DeduplicationPhase::<Text>::deduplicate(
            &DeduplicationParams::default(),
            &FilterParams::default(),
            entities,
            &target,
        )
        .await;
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence.get() - 0.85).abs() < f64::EPSILON);
    }
}
