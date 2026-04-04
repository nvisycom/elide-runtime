//! Execution behaviour for [`DeduplicationStrategy`].
//!
//! Implements the actual confidence-combination algorithms and entity
//! field merging. The strategy determines *how* confidences are combined;
//! the grouping module determines *which* entities are candidates.

use std::collections::HashSet;

use nvisy_ontology::entity::{Entities, Entity, RefinementMethod};
use nvisy_ontology::workflow::{DeduplicationStrategy, GroupingCriteria};

use super::grouping::GroupEntities;

const TARGET: &str = "nvisy_engine::op::deduplication::strategy";

/// Adds deduplication execution to [`DeduplicationStrategy`].
pub(super) trait DeduplicationStrategyExt {
    /// Group entities by the given criteria, then fuse each group into a
    /// single entity.
    #[must_use]
    fn fuse(&self, entities: Entities, criteria: GroupingCriteria) -> Entities;

    /// Fuse a group of co-referent entities into one.
    ///
    /// Field merging rules:
    /// - **base**: highest-confidence entity (most trusted metadata)
    /// - **value**: longest value wins (more specific match)
    /// - **location**: follows the selected value
    /// - **recognition/extraction methods**: order-preserving union
    /// - **language/model**: first non-`None` across the group
    /// - **confidence**: computed by the strategy
    #[must_use]
    fn fuse_group(&self, group: Vec<Entity>) -> Entity;

    /// Compute the fused confidence for a group of entities.
    #[must_use]
    fn compute_confidence(&self, group: &[Entity]) -> f64;
}

impl DeduplicationStrategyExt for DeduplicationStrategy {
    fn fuse(&self, entities: Entities, criteria: GroupingCriteria) -> Entities {
        if entities.len() <= 1 {
            return entities;
        }

        let groups = entities.group(criteria);

        tracing::debug!(
            target: TARGET,
            strategy = ?self,
            groups = groups.len(),
            "fusing entity groups",
        );

        groups
            .into_iter()
            .map(|group| self.fuse_group(group))
            .collect()
    }

    fn fuse_group(&self, mut group: Vec<Entity>) -> Entity {
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

        // Prefer the longest value: it is the more specific match
        // (e.g. "John Smith" over "John"). When the value changes,
        // adopt the location from the entity that produced it so the
        // span stays consistent.
        for e in &rest {
            if e.value.len() > result.value.len() {
                result.value.clone_from(&e.value);
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

        tracing::trace!(
            target: TARGET,
            entity_id = %result.id,
            fused_from = rest.len() + 1,
            confidence = fused_confidence,
            ?refinement,
            value = %result.value,
            "fused entity group",
        );

        result
    }

    fn compute_confidence(&self, group: &[Entity]) -> f64 {
        match self {
            Self::MaxConfidence => group.iter().map(|e| e.confidence).fold(0.0_f64, f64::max),
            Self::WeightedAverage { weights } => {
                // For each entity, use the max weight across all its
                // recognition methods. Methods absent from the weight
                // map default to 1.0.
                let mut total_weight = 0.0;
                let mut weighted_sum = 0.0;
                for e in group {
                    let w = e
                        .recognition_methods
                        .iter()
                        .filter_map(|m| weights.get(&m.kind()).copied())
                        .fold(1.0_f64, f64::max);
                    weighted_sum += e.confidence * w;
                    total_weight += w;
                }
                if total_weight > 0.0 {
                    weighted_sum / total_weight
                } else {
                    0.0
                }
            }
            Self::NoisyOr => {
                // Independent-detector combination:
                // P(any) = 1 - ∏(1 - pᵢ)
                let product: f64 = group.iter().map(|e| 1.0 - e.confidence).product();
                1.0 - product
            }
            // Fallback for future variants: treat as MaxConfidence.
            _ => group.iter().map(|e| e.confidence).fold(0.0_f64, f64::max),
        }
    }
}

/// Classify whether a group merge is a deduplication (same detector
/// kinds) or an ensemble fusion (different detector kinds).
fn classify_refinement(group: &[Entity]) -> RefinementMethod {
    let first_kinds: HashSet<_> = group[0]
        .recognition_methods
        .iter()
        .map(|m| m.kind())
        .collect();

    let all_same = group[1..].iter().all(|e| {
        let kinds: HashSet<_> = e.recognition_methods.iter().map(|m| m.kind()).collect();
        kinds == first_kinds
    });

    if all_same {
        RefinementMethod::Deduplication
    } else {
        RefinementMethod::EnsembleFusion
    }
}
