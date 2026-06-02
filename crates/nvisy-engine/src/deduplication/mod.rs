//! Entity deduplication: four-step pipeline (calibrate → filter →
//! fuse → resolve) over a flat list of entities for one document.
//!
//! Public surface is the [`deduplicate`] free function plus
//! [`DeduplicationParams`], the strategy/criteria types, and the
//! [`SpanSize`] / [`CalibrationMap`] helpers consumed by them. The
//! phase orchestrator that drives this per [`DocumentTree`] node
//! lives in [`crate::pipeline::phases::DeduplicationPhase`].
//!
//! Step ordering:
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
//!
//! [`DocumentTree`]: crate::core::DocumentTree

mod calibrate;
mod filter;
mod fuse;
mod resolve;
mod span_size;

use nvisy_ontology::document::Document;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::{Modality, Overlap};

use self::calibrate::Calibrate;
pub use self::calibrate::CalibrationMap;
use self::filter::Filter;
pub use self::filter::FilterParams;
use self::fuse::Fuse;
pub use self::fuse::{DeduplicationStrategy, GroupingCriteria};
pub use self::resolve::ConflictResolution;
use self::resolve::ResolveConflicts;
pub use self::span_size::SpanSize;
use crate::core::{DocumentView, SharedHandle, ValueAt};
use crate::pipeline::DeduplicationParams;

const TARGET: &str = "nvisy_engine::deduplication";

/// Run the four-step pipeline against an owned entity list. Shared
/// by [`DeduplicationPhase::apply`] and the test harness so both go
/// through the same code path.
///
/// [`DeduplicationPhase::apply`]: crate::pipeline::DeduplicationPhase::apply
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nvisy_ontology::document::Document;
    use nvisy_ontology::entity::{Entity, ModelProvenance, TrailProvenance, TrailStep};
    use nvisy_ontology::modality::{Text, TextExtraction, TextMetadata};
    use nvisy_ontology::primitive::{Confidence, ConfidenceThreshold};
    use tokio::sync::Mutex;

    use super::*;
    use crate::core::SharedHandle;

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
        let result = deduplicate::<Text>(
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
        let result = deduplicate::<Text>(
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
