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
mod resolve;
mod span_size;

use std::mem;

use nvisy_core::Result;
use nvisy_ontology::document::Document;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::{Modality, Overlap};
use nvisy_ontology::provenance::EntityRecord;
use tracing::Instrument;

use self::calibrate::Calibrate;
pub use self::calibrate::CalibrationMap;
use self::filter::Filter;
pub use self::filter::FilterParams;
use self::fuse::Fuse;
pub use self::fuse::{DeduplicationStrategy, GroupingCriteria};
pub use self::resolve::ConflictResolution;
use self::resolve::ResolveConflicts;
pub use self::span_size::SpanSize;
use crate::core::{DocumentTree, DocumentView, NodeMut, RunContext, SharedHandle, ValueAt};
use crate::pipeline::{DeduplicationParams, Detection, EngineInput};

const TARGET: &str = "nvisy_engine::deduplication";

/// Deduplication phase: calibrate raw confidence scores, filter
/// below-threshold or wrong-kind entities, group + fuse overlapping
/// matches, then resolve cross-kind conflicts.
///
/// Stateless. Per-run config ([`DeduplicationParams`] for
/// calibration/threshold/grouping, [`Detection`] for the
/// allowed-kinds list) comes from `input.plan` each call.
pub struct DeduplicationPhase;

impl DeduplicationPhase {
    /// Build the phase. Stateless.
    pub fn new() -> Self {
        Self
    }

    /// Walk the tree and run the per-node dedup body. Visits the
    /// root first, then iterates nested embedded documents; each
    /// per-node body borrows the detection + dedup plan and handle
    /// directly from this scope.
    pub(crate) async fn apply(
        &self,
        _ctx: &RunContext,
        input: &EngineInput,
        tree: &mut DocumentTree,
    ) -> Result<()> {
        let span = tracing::info_span!(target: TARGET, "phase", name = "deduplication");
        // Snapshot the tree-owned handle so it doesn't conflict with
        // the per-node `&mut` borrows produced by `root_mut` /
        // `embeds_mut` further down.
        let handle = tree.handle.clone();
        async move {
            dispatch(
                tree.root_mut(),
                &handle,
                &input.plan.deduplication,
                &input.plan.detection,
            )
            .await?;
            for node in tree.embeds_mut() {
                dispatch(
                    node,
                    &handle,
                    &input.plan.deduplication,
                    &input.plan.detection,
                )
                .await?;
            }
            Ok(())
        }
        .instrument(span)
        .await
    }

    /// Run the four-step pipeline against an owned entity list.
    /// Shared by [`Self::apply`] and the test harness so both go
    /// through the same code path.
    pub(crate) async fn deduplicate<M>(
        params: &DeduplicationParams,
        filter: &FilterParams,
        mut entities: Vec<Entity<M>>,
        doc: &Document<M>,
        handle: &SharedHandle,
    ) -> Vec<Entity<M>>
    where
        M: Modality + Overlap + SpanSize,
        for<'a> DocumentView<'a, M>: ValueAt<M>,
    {
        if entities.is_empty() {
            return entities;
        }
        let before = entities.len();

        let view = DocumentView::new(doc, handle);
        entities.calibrate(&params.calibration);
        let dropped = entities.filter(filter);
        entities
            .fuse(&params.strategy, params.grouping, &view)
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

impl Default for DeduplicationPhase {
    fn default() -> Self {
        Self::new()
    }
}

async fn dispatch(
    node: NodeMut<'_>,
    handle: &SharedHandle,
    dedup: &DeduplicationParams,
    detection: &Detection,
) -> Result<()> {
    match node {
        NodeMut::Text(doc) => dedup_one(doc, handle, dedup, detection).await,
        NodeMut::Tabular(doc) => dedup_one(doc, handle, dedup, detection).await,
        NodeMut::Image(doc) => dedup_one(doc, handle, dedup, detection).await,
        NodeMut::Audio(doc) => dedup_one(doc, handle, dedup, detection).await,
    }
}

async fn dedup_one<M>(
    doc: &mut Document<M>,
    handle: &SharedHandle,
    dedup: &DeduplicationParams,
    detection: &Detection,
) -> Result<()>
where
    M: Modality + Overlap + SpanSize,
    for<'a> DocumentView<'a, M>: ValueAt<M>,
{
    if doc.audit.records.is_empty() {
        return Ok(());
    }
    let filter = FilterParams {
        allowed_kinds: (!detection.entity_kinds.is_empty()).then(|| detection.entity_kinds.clone()),
        confidence_threshold: dedup.confidence_threshold,
    };
    tracing::debug!(
        target: TARGET,
        entities = doc.audit.records.len(),
        "running deduplication",
    );
    // Dedup runs before redaction evaluation, so every record's
    // `audit` is still None; we can pull entities out, dedup, and
    // rewrap without losing audit state.
    let records = mem::take(&mut doc.audit.records);
    let entities: Vec<Entity<M>> = records.into_iter().map(|r| r.entity).collect();
    let deduped = DeduplicationPhase::deduplicate(dedup, &filter, entities, doc, handle).await;
    doc.audit.records = deduped.into_iter().map(EntityRecord::new).collect();
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nvisy_ontology::document::Document;
    use nvisy_ontology::entity::{Entity, ModelProvenance, TrailProvenance, TrailStep};
    use nvisy_ontology::modality::{Text, TextExtraction, TextMetadata};
    use nvisy_ontology::primitive::{Confidence, ConfidenceThreshold};
    use tokio::sync::Mutex;

    use super::*;
    use crate::core::{SharedHandle, ValueAt};

    fn conf(v: f64) -> Confidence {
        Confidence::new(v).expect("confidence in [0,1]")
    }

    const TEST_TEXT: &str = "John Smith";

    /// Owned per-test components used to drive the dedup pipeline.
    struct Fixture {
        handle: SharedHandle,
        doc: Document<Text>,
    }

    async fn test_fixture(text: &str) -> Fixture {
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
        Fixture { handle, doc }
    }

    #[tokio::test]
    async fn confidence_threshold_filters() {
        let fix = test_fixture("John......Jane").await;
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
        let result = DeduplicationPhase::deduplicate::<Text>(
            &DeduplicationParams::default(),
            &filter,
            entities,
            &fix.doc,
            &fix.handle,
        )
        .await;
        assert_eq!(result.len(), 1);
        let view = DocumentView::new(&fix.doc, &fix.handle);
        let value = view.value_at(&result[0].location).await;
        assert_eq!(value.as_deref(), Some("John"));
    }

    #[tokio::test]
    async fn full_pipeline() {
        let fix = test_fixture(TEST_TEXT).await;
        let entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.7))
                .test_build(),
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.8))
                .test_build(),
            Entity::test_builder(0, 4)
                .with_trail(vec![TrailStep::recognition(
                    "ner",
                    conf(0.85),
                    TrailProvenance::Model(ModelProvenance::new("test")),
                    "",
                )])
                .with_confidence(conf(0.85))
                .test_build(),
        ];
        let result = DeduplicationPhase::deduplicate::<Text>(
            &DeduplicationParams::default(),
            &FilterParams::default(),
            entities,
            &fix.doc,
            &fix.handle,
        )
        .await;
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence.get() - 0.85).abs() < f64::EPSILON);
    }
}
