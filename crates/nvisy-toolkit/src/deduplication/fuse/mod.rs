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
use nvisy_core::entity::{Entity, TrailStep};
use nvisy_core::extraction::TextAt;
use nvisy_core::modality::{Modality, Overlap};
use nvisy_core::primitive::Confidence;

use self::group::GroupEntities;
pub use self::group::GroupingCriteria;
pub use self::strategy::DeduplicationStrategy;
use super::layer::{Layer, LayerContext};
use super::span_size::SpanSize;

const TARGET: &str = "nvisy_document::deduplication::fuse";

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
    M: Modality,
    M::Location: Overlap + SpanSize,
    R: TextAt<M> + ?Sized,
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
async fn fuse_group<M, V: TextAt<M> + ?Sized>(
    strategy: &DeduplicationStrategy,
    mut group: Vec<Entity<M>>,
    view: &V,
) -> Entity<M>
where
    M: Modality,
    M::Location: SpanSize,
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

    let value = view.text_at(&result.location).await.unwrap_or_default();
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
