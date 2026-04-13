//! Execution behaviour for [`DeduplicationStrategy`].
//!
//! Implements the actual confidence-combination algorithms and entity
//! field merging. The strategy determines *how* confidences are combined;
//! the grouping module determines *which* entities are candidates.

use std::collections::HashSet;

use nvisy_ontology::entity::{Entities, Entity, RefinementMethod};
use nvisy_ontology::workflow::{DeduplicationStrategy, GroupingCriteria};

use super::grouping::GroupEntities;
use super::span_size::SpanSize;
use crate::operation::Document;

const TARGET: &str = "nvisy_engine::op::deduplication::strategy";

/// Adds deduplication execution to [`DeduplicationStrategy`].
pub(super) trait DeduplicationStrategyExt {
    /// Group entities by the given criteria, then fuse each group into a
    /// single entity.
    fn fuse(
        &self,
        entities: Entities,
        criteria: GroupingCriteria,
        document: &Document,
    ) -> impl Future<Output = Entities> + Send;

    /// Fuse a group of co-referent entities into one.
    fn fuse_group(
        &self,
        group: Vec<Entity>,
        document: &Document,
    ) -> impl Future<Output = Entity> + Send;

    /// Compute the fused confidence for a group of entities.
    #[must_use]
    fn compute_confidence(&self, group: &[Entity]) -> f64;
}

impl DeduplicationStrategyExt for DeduplicationStrategy {
    async fn fuse(
        &self,
        entities: Entities,
        criteria: GroupingCriteria,
        document: &Document,
    ) -> Entities {
        if entities.len() <= 1 {
            return entities;
        }

        let groups = entities.group(criteria, document).await;

        tracing::debug!(
            target: TARGET,
            strategy = ?self,
            groups = groups.len(),
            "fusing entity groups",
        );

        let mut result = Entities::new();
        for group in groups {
            result.push(self.fuse_group(group, document).await);
        }
        result
    }

    async fn fuse_group(&self, mut group: Vec<Entity>, document: &Document) -> Entity {
        debug_assert!(!group.is_empty());

        if group.len() == 1 {
            return group.into_iter().next().unwrap();
        }

        let fused_confidence = self.compute_confidence(&group);

        // Determine the refinement type: if all entities in the group
        // share the same set of recognition method kinds, this is a
        // deduplication (same detector produced duplicates). Otherwise
        // it's an ensemble fusion (different detectors combined).
        let refinement = classify_refinement(&group);

        // Sort by descending confidence: highest-confidence entity
        // becomes the base since it carries the most trusted metadata.
        group.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
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
                .unwrap_or(std::cmp::Ordering::Less)
                == std::cmp::Ordering::Greater
            {
                result.location.clone_from(&e.location);
            }
        }

        // Merge recognition methods (order-preserving union).
        let mut seen_rec: HashSet<_> = result.recognition_methods.iter().cloned().collect();
        for e in &rest {
            for m in &e.recognition_methods {
                if seen_rec.insert(m.clone()) {
                    result.recognition_methods.push(m.clone());
                }
            }
        }

        // Merge extraction methods (order-preserving union).
        let mut seen_ext: HashSet<_> = result.extraction_methods.iter().cloned().collect();
        for e in &rest {
            for m in &e.extraction_methods {
                if seen_ext.insert(*m) {
                    result.extraction_methods.push(*m);
                }
            }
        }

        // Fill in missing optional fields from lower-confidence entities.
        if result.language.is_none() {
            result.language = rest.iter().find_map(|e| e.language.clone());
        }
        result.confidence = fused_confidence;
        result.refinement_methods.push(refinement);

        let value = document
            .value_at(&result.location)
            .await
            .unwrap_or_default();
        tracing::trace!(
            target: TARGET,
            entity_id = %result.id,
            fused_from = rest.len() + 1,
            confidence = fused_confidence,
            ?refinement,
            value,
            "fused entity group",
        );

        result
    }

    fn compute_confidence(&self, group: &[Entity]) -> f64 {
        match self {
            Self::MaxConfidence => group
                .iter()
                .map(|e| e.confidence)
                .fold(f64::NEG_INFINITY, f64::max),

            Self::NoisyOr => {
                // P(at least one) = 1 − ∏(1 − pᵢ)
                1.0 - group.iter().map(|e| 1.0 - e.confidence).product::<f64>()
            }

            Self::WeightedAverage { weights } => {
                let (wsum, total_w) =
                    group.iter().fold((0.0_f64, 0.0_f64), |(wsum, total_w), e| {
                        let w = e
                            .recognition_methods
                            .iter()
                            .filter_map(|m| weights.get(&m.kind()))
                            .copied()
                            .fold(1.0_f64, f64::max);
                        (wsum + e.confidence * w, total_w + w)
                    });
                if total_w > 0.0 { wsum / total_w } else { 0.0 }
            }
            _ => group
                .iter()
                .map(|e| e.confidence)
                .fold(f64::NEG_INFINITY, f64::max),
        }
    }
}

/// Classify how the group was formed (all same detector kind → Dedup,
/// mixed → Ensemble).
fn classify_refinement(group: &[Entity]) -> RefinementMethod {
    let first_kinds: HashSet<_> = group[0]
        .recognition_methods
        .iter()
        .map(|m| m.kind())
        .collect();
    let all_same = group.iter().skip(1).all(|e| {
        e.recognition_methods
            .iter()
            .map(|m| m.kind())
            .collect::<HashSet<_>>()
            == first_kinds
    });
    if all_same {
        RefinementMethod::Deduplication
    } else {
        RefinementMethod::EnsembleFusion
    }
}
