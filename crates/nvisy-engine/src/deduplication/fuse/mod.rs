//! Group + fuse co-referent entities into a single detection.
//!
//! Implements the actual confidence-combination algorithms and entity
//! field merging. The strategy determines *how* confidences are
//! combined; the grouping module determines *which* entities are
//! candidates.

mod group;
mod key;
mod strategy;

use std::cmp::Ordering;
use std::collections::HashSet;

use async_trait::async_trait;
use nvisy_ontology::entity::{Entity, TrailStep};
use nvisy_ontology::modality::{Modality, Overlap};
use nvisy_ontology::primitive::Confidence;

use self::group::GroupEntities;
pub use self::group::GroupingCriteria;
pub use self::strategy::DeduplicationStrategy;
use super::layer::{Layer, LayerContext};
use super::span_size::SpanSize;
use crate::core::ValueAt;

const TARGET: &str = "nvisy_engine::deduplication::fuse";

/// [`Layer`] that groups co-referent entities and merges each group
/// into a single entity.
///
/// Drops nothing — but reshapes the collection (each group of N
/// entities becomes one). Returns an empty vec from [`Layer::apply`].
pub struct FuseLayer {
    strategy: DeduplicationStrategy,
    criteria: GroupingCriteria,
}

impl FuseLayer {
    /// Construct a fuse layer from a strategy + criteria.
    pub fn new(strategy: DeduplicationStrategy, criteria: GroupingCriteria) -> Self {
        Self { strategy, criteria }
    }
}

#[async_trait]
impl<M, R> Layer<M, R> for FuseLayer
where
    M: Modality + Overlap + SpanSize,
    R: ValueAt<M> + ?Sized,
{
    async fn apply(
        &self,
        entities: &mut Vec<Entity<M>>,
        ctx: &LayerContext<'_, M, R>,
    ) -> Vec<Entity<M>> {
        if entities.len() <= 1 {
            return Vec::new();
        }

        let owned = std::mem::take(entities);
        let groups = owned.group(self.criteria, ctx.resolver).await;

        tracing::debug!(
            target: TARGET,
            strategy = ?self.strategy,
            groups = groups.len(),
            "fusing entity groups",
        );

        for group in groups {
            entities.push(fuse_group(&self.strategy, group, ctx.resolver).await);
        }

        Vec::new()
    }
}

/// Fuse a group of co-referent entities into one.
async fn fuse_group<M, V: ValueAt<M> + ?Sized>(
    strategy: &DeduplicationStrategy,
    mut group: Vec<Entity<M>>,
    view: &V,
) -> Entity<M>
where
    M: Modality + SpanSize,
{
    debug_assert!(!group.is_empty());

    if group.len() == 1 {
        return group
            .into_iter()
            .next()
            .expect("group.len() == 1 by guard above");
    }

    let fused_confidence = strategy.compute_confidence(&group);

    // Classify: if every entity in the group has the same set of
    // recognizer source names, this is plain deduplication (same
    // detector produced duplicates); otherwise it's ensemble fusion.
    let label = classify_fusion(&group);

    // Sort by descending confidence: highest-confidence entity
    // becomes the base since it carries the most trusted metadata.
    group.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(Ordering::Equal)
    });
    let mut result = group.remove(0);
    let rest = group;

    // Prefer the largest span: for text, the longer match is more
    // specific (e.g. "John Smith" over "John"); for images, the
    // larger bounding box. Adopt the winner's location so the
    // span stays consistent.
    for e in &rest {
        if e.location
            .span_cmp(&result.location)
            .unwrap_or(Ordering::Less)
            == Ordering::Greater
        {
            result.location.clone_from(&e.location);
        }
    }

    // Merge trails (order-preserving union of every step in every
    // entity, deduplicated by exact equality).
    for e in &rest {
        for step in &e.trail {
            if !result.trail.contains(step) {
                result.trail.push(step.clone());
            }
        }
    }

    let before = result.confidence;
    let after = Confidence::new(fused_confidence.clamp(0.0, 1.0)).expect("clamped to [0,1]");
    result.confidence = after;
    let reason = format!("{label} of {} entities", rest.len() + 1);
    result.trail.push(TrailStep::fusion(before, after, reason));

    let value = view.value_at(&result.location).await.unwrap_or_default();
    tracing::trace!(
        target: TARGET,
        entity_id = %result.id,
        fused_from = rest.len() + 1,
        confidence = fused_confidence,
        label,
        value,
        "fused entity group",
    );

    result
}

/// Label the group as plain deduplication or ensemble fusion. Used
/// only for the trail step's `reason` text — no behaviour depends on
/// this discriminant.
fn classify_fusion<M: Modality>(group: &[Entity<M>]) -> &'static str {
    let first: HashSet<&str> = group[0].recognizers().collect();
    let all_same = group
        .iter()
        .skip(1)
        .all(|e| e.recognizers().collect::<HashSet<_>>() == first);
    if all_same {
        "deduplication"
    } else {
        "ensemble fusion"
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use nvisy_core::content::ContentMetadata;
    use nvisy_formats::test_utils::decode_text;
    use nvisy_ontology::document::Document;
    use nvisy_ontology::entity::{Entity, ModelProvenance, TrailProvenance, TrailStepKind};
    use nvisy_ontology::modality::{Text, TextExtraction, TextMetadata};
    use nvisy_ontology::primitive::Confidence;
    use tokio::sync::Mutex;

    use super::*;
    use crate::core::{DocumentView, SharedData, SharedHandle};
    use crate::ingestion::registry::Registry;

    /// Replace the entity's recognition trail step with one stamped
    /// against `source` — used by tests to simulate entities produced
    /// by different recognizers without going through the full
    /// detection path.
    fn ner_step(confidence: Confidence) -> TrailStep {
        TrailStep::recognition(
            "ner",
            confidence,
            TrailProvenance::Model(ModelProvenance::new("test")),
            "",
        )
    }

    /// Build per-test owned components ready to construct a
    /// `DocumentView` against. Tests build this once and create
    /// fresh views each invocation.
    async fn test_fixture(
        text: &str,
    ) -> (
        SharedHandle,
        Document<Text>,
        ContentMetadata,
        Arc<SharedData>,
    ) {
        let handle: SharedHandle =
            Arc::new(Mutex::new(decode_text(text).await.expect("decode text")));
        let source = handle.lock().await.source();
        let doc = Document::<Text>::new(
            TextMetadata {
                extraction: TextExtraction::Native,
                languages: Vec::new(),
            },
            source,
        );
        let metadata = ContentMetadata::new().with_content_type("text/plain");
        let registry =
            Registry::open(tempfile::tempdir().expect("tempdir").path()).expect("open registry");
        let shared = SharedData::new(uuid::Uuid::nil(), uuid::Uuid::nil(), registry);
        (handle, doc, metadata, shared)
    }

    fn conf(v: f64) -> Confidence {
        Confidence::new(v).expect("confidence in [0,1]")
    }

    async fn fuse_with(
        strategy: DeduplicationStrategy,
        criteria: GroupingCriteria,
        view: &DocumentView<'_, Text>,
        entities: &mut Vec<Entity<Text>>,
    ) {
        let ctx = LayerContext::new(view);
        let layer = FuseLayer::new(strategy, criteria);
        let dropped = layer.apply(entities, &ctx).await;
        assert!(dropped.is_empty(), "fuse never drops");
    }

    const TEXT: &str = "John Smith";

    /// Two entities at the same `(start, end)` byte range fuse into
    /// one. With `MaxConfidence`, the higher of the two scores
    /// (0.8 vs the default 0.9 from `test_build`) wins.
    #[tokio::test]
    async fn strict_grouping_fuses_identical_spans_with_max_confidence() {
        let (handle, doc, _metadata, _shared) = test_fixture(TEXT).await;
        let view = DocumentView::new(&doc, &handle);
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.8))
                .test_build(),
            Entity::test_builder(0, 4).test_build(),
        ];
        fuse_with(
            DeduplicationStrategy::MaxConfidence,
            GroupingCriteria::Strict,
            &view,
            &mut entities,
        )
        .await;
        assert_eq!(entities.len(), 1);
        assert!((entities[0].confidence.get() - 0.9).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn narrowing_groups_substring_with_overlap() {
        let (handle, doc, _metadata, _shared) = test_fixture(TEXT).await;
        let view = DocumentView::new(&doc, &handle);
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.8))
                .test_build(),
            Entity::test_builder(0, 10)
                .with_trail(vec![ner_step(conf(0.9))])
                .test_build(),
        ];
        fuse_with(
            DeduplicationStrategy::MaxConfidence,
            GroupingCriteria::Narrowing,
            &view,
            &mut entities,
        )
        .await;
        assert_eq!(entities.len(), 1);
        let value = view.value_at(&entities[0].location).await;
        assert_eq!(value.as_deref(), Some("John Smith"));
    }

    #[tokio::test]
    async fn widening_groups_across_non_overlapping_locations() {
        let text = format!("{:<100}John Smith", TEXT);
        let (handle, doc, _metadata, _shared) = test_fixture(&text).await;
        let view = DocumentView::new(&doc, &handle);
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4).test_build(),
            Entity::test_builder(100, 110)
                .with_trail(vec![ner_step(conf(0.9))])
                .test_build(),
        ];
        fuse_with(
            DeduplicationStrategy::MaxConfidence,
            GroupingCriteria::Widening,
            &view,
            &mut entities,
        )
        .await;
        assert_eq!(entities.len(), 1);
    }

    #[tokio::test]
    async fn noisy_or_strategy() {
        let (handle, doc, _metadata, _shared) = test_fixture(TEXT).await;
        let view = DocumentView::new(&doc, &handle);
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.7))
                .test_build(),
            Entity::test_builder(0, 4)
                .with_trail(vec![ner_step(conf(0.9))])
                .with_confidence(conf(0.8))
                .test_build(),
        ];
        fuse_with(
            DeduplicationStrategy::NoisyOr,
            GroupingCriteria::default(),
            &view,
            &mut entities,
        )
        .await;
        assert_eq!(entities.len(), 1);
        // 1 - (1 - 0.7)(1 - 0.8) = 1 - 0.06 = 0.94
        assert!((entities[0].confidence.get() - 0.94).abs() < 0.001);
    }

    #[tokio::test]
    async fn weighted_average_strategy() {
        let (handle, doc, _metadata, _shared) = test_fixture(TEXT).await;
        let view = DocumentView::new(&doc, &handle);
        let mut weights = HashMap::new();
        weights.insert("pattern".to_string(), 1.0);
        weights.insert("ner".to_string(), 2.0);

        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.6))
                .test_build(),
            Entity::test_builder(0, 4)
                .with_trail(vec![ner_step(conf(0.9))])
                .test_build(),
        ];
        fuse_with(
            DeduplicationStrategy::WeightedAverage { weights },
            GroupingCriteria::default(),
            &view,
            &mut entities,
        )
        .await;
        assert_eq!(entities.len(), 1);
        // (0.6 * 1.0 + 0.9 * 2.0) / 3.0 = 0.8
        assert!((entities[0].confidence.get() - 0.8).abs() < 0.001);
    }

    #[tokio::test]
    async fn different_detector_tagged_as_ensemble_fusion() {
        let (handle, doc, _metadata, _shared) = test_fixture(TEXT).await;
        let view = DocumentView::new(&doc, &handle);
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.8))
                .test_build(),
            Entity::test_builder(0, 4)
                .with_trail(vec![ner_step(conf(0.9))])
                .test_build(),
        ];
        fuse_with(
            DeduplicationStrategy::MaxConfidence,
            GroupingCriteria::default(),
            &view,
            &mut entities,
        )
        .await;
        assert_eq!(entities.len(), 1);
        assert!(
            entities[0]
                .trail
                .iter()
                .any(|s| matches!(s.kind, TrailStepKind::Fusion) && s.reason.contains("ensemble"))
        );
    }
}
