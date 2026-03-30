//! Execution behavior for [`FusionStrategy`].

use std::collections::HashSet;

use nvisy_ontology::entity::{Entities, Entity, RefinementMethod};
use nvisy_ontology::workflow::{FusionStrategy, GroupingCriteria};

use super::grouping::GroupEntities;

/// Execution behavior for [`FusionStrategy`].
pub(super) trait FusionStrategyExt {
    /// Group entities then fuse each group into a single entity.
    fn fuse(&self, entities: Entities, criteria: GroupingCriteria) -> Entities;

    /// Fuse a group of matching entities into a single entity.
    fn fuse_group(&self, group: Vec<Entity>) -> Entity;

    /// Compute fused confidence for a group of entities.
    fn compute_confidence(&self, group: &[Entity]) -> f64;
}

impl FusionStrategyExt for FusionStrategy {
    fn fuse(&self, entities: Entities, criteria: GroupingCriteria) -> Entities {
        if entities.len() <= 1 {
            return entities;
        }

        entities
            .group(criteria)
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

        // Pick the highest-confidence entity as the base.
        group.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut result = group.remove(0);
        let rest = group;

        // Use the longest value (more specific match).
        for e in &rest {
            if e.value.len() > result.value.len() {
                result.value.clone_from(&e.value);
                result.location.clone_from(&e.location);
            }
        }

        // Merge recognition methods (order-preserving dedup).
        let mut seen_rec: HashSet<_> = result.recognition_methods.iter().copied().collect();
        for e in &rest {
            for m in &e.recognition_methods {
                if seen_rec.insert(*m) {
                    result.recognition_methods.push(*m);
                }
            }
        }

        // Merge extraction methods.
        let mut seen_ext: HashSet<_> = result.extraction_methods.iter().copied().collect();
        for e in &rest {
            for m in &e.extraction_methods {
                if seen_ext.insert(*m) {
                    result.extraction_methods.push(*m);
                }
            }
        }

        // Fill in missing optional fields from other entities.
        if result.language.is_none() {
            result.language = rest.iter().find_map(|e| e.language.clone());
        }
        if result.model.is_none() {
            result.model = rest.iter().find_map(|e| e.model.clone());
        }

        result.confidence = fused_confidence;
        result
            .refinement_methods
            .push(RefinementMethod::EnsembleFusion);
        result
    }

    fn compute_confidence(&self, group: &[Entity]) -> f64 {
        match self {
            Self::MaxConfidence => group.iter().map(|e| e.confidence).fold(0.0_f64, f64::max),
            Self::WeightedAverage { weights } => {
                let mut total_weight = 0.0;
                let mut weighted_sum = 0.0;
                for e in group {
                    let w = e
                        .recognition_methods
                        .iter()
                        .filter_map(|m| weights.get(m).copied())
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
                let product: f64 = group.iter().map(|e| 1.0 - e.confidence).product();
                1.0 - product
            }
            _ => group.iter().map(|e| e.confidence).fold(0.0_f64, f64::max),
        }
    }
}
