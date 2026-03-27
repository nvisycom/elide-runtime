//! Fusion node configuration and strategy.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::{Entities, Entity, Overlap, RecognitionMethod, RefinementMethod};

/// Strategy for combining confidence scores from multiple detectors.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FusionStrategy {
    /// Take the maximum confidence across all detectors.
    #[default]
    MaxConfidence,
    /// Weighted average by recognition method.
    WeightedAverage {
        /// Per-method weight (missing methods default to 1.0).
        weights: HashMap<RecognitionMethod, f64>,
    },
    /// Noisy-OR: `P = 1 − ∏(1 − pᵢ)` for independent detectors.
    NoisyOr,
}

impl FusionStrategy {
    /// Group entities by `(kind, value, overlapping location)` then fuse
    /// each group into a single entity using this strategy.
    pub fn fuse(&self, entities: Entities) -> Entities {
        if entities.len() <= 1 {
            return entities;
        }

        let mut groups: Vec<Vec<Entity>> = Vec::new();

        for entity in entities {
            let group = groups.iter_mut().find(|group| {
                let rep = &group[0];
                rep.entity_kind == entity.entity_kind
                    && rep.value == entity.value
                    && rep.location.overlaps(&entity.location)
            });

            match group {
                Some(g) => g.push(entity),
                None => groups.push(vec![entity]),
            }
        }

        groups
            .into_iter()
            .map(|group| self.fuse_group(group))
            .collect()
    }

    /// Fuse a group of matching entities into a single entity using this strategy.
    pub fn fuse_group(&self, group: Vec<Entity>) -> Entity {
        debug_assert!(!group.is_empty());

        if group.len() == 1 {
            return group.into_iter().next().unwrap();
        }

        let fused_confidence = match self {
            Self::MaxConfidence => group.iter().map(|e| e.confidence).fold(0.0_f64, f64::max),
            Self::WeightedAverage { weights } => {
                let mut total_weight = 0.0;
                let mut weighted_sum = 0.0;
                for e in &group {
                    let w = e
                        .recognition_methods
                        .first()
                        .and_then(|m| weights.get(m))
                        .copied()
                        .unwrap_or(1.0);
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
        };

        let mut merged_methods = Vec::new();
        for e in &group {
            for m in &e.recognition_methods {
                if !merged_methods.contains(m) {
                    merged_methods.push(*m);
                }
            }
        }

        let mut result = group.into_iter().next().unwrap();
        result.confidence = fused_confidence;
        result.recognition_methods = merged_methods;
        result
            .refinement_methods
            .push(RefinementMethod::EnsembleFusion);
        result
    }
}

/// Configuration for the `Fusion` graph node.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Fusion {
    /// Remove overlapping duplicate entities before fusion.
    #[serde(default)]
    pub entity_deduplication: bool,
    /// Strategy for combining confidence scores.
    #[serde(default)]
    pub strategy: FusionStrategy,
    /// Adjust raw model scores to align with empirical precision targets.
    #[serde(default)]
    pub confidence_calibration: bool,
    /// Use surrounding document context to upgrade or downgrade confidence.
    #[serde(default)]
    pub contextual_adjustment: bool,
}
