//! Ensemble entity fusion: merges entities from multiple detectors
//! using configurable confidence-combination strategies.

use std::collections::HashMap;

use nvisy_core::Result;
use nvisy_ontology::entity::{DetectionMethod, Entity, Location};

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::ensemble";

/// Strategy for combining confidence scores from multiple detectors.
#[derive(Debug, Clone)]
pub enum FusionStrategy {
    /// Take the maximum confidence across all detectors.
    MaxConfidence,
    /// Weighted average by detection method.
    WeightedAverage {
        weights: HashMap<DetectionMethod, f64>,
    },
    /// Noisy-OR: `P = 1 − ∏(1 − pᵢ)` for independent detectors.
    NoisyOr,
}

/// Ensemble merge: groups entities by `(kind, value, overlapping location)`
/// then fuses confidence using the configured [`FusionStrategy`].
pub struct Ensemble {
    strategy: FusionStrategy,
}

impl Ensemble {
    /// Create a new ensemble merge with the given strategy.
    pub fn new(strategy: FusionStrategy) -> Self {
        Self { strategy }
    }

    async fn fuse(&self, entities: Vec<Entity>) -> Result<Vec<Entity>> {
        let before = entities.len();
        let result = self.merge(entities);
        tracing::debug!(target: TARGET, before, after = result.len(), "fused entities");
        Ok(result)
    }

    /// Group entities by `(kind, value, overlapping location)` then fuse
    /// confidence according to the strategy.
    pub fn merge(&self, entities: Vec<Entity>) -> Vec<Entity> {
        if entities.len() <= 1 {
            return entities;
        }

        let mut groups: Vec<Vec<Entity>> = Vec::new();

        for entity in entities {
            let group = groups.iter_mut().find(|group| {
                let representative = &group[0];
                representative.entity_kind == entity.entity_kind
                    && representative.value == entity.value
                    && locations_overlap(&representative.location, &entity.location)
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

    /// Fuse a group of matching entities into a single entity.
    fn fuse_group(&self, group: Vec<Entity>) -> Entity {
        debug_assert!(!group.is_empty());

        if group.len() == 1 {
            return group.into_iter().next().unwrap();
        }

        let fused_confidence = match &self.strategy {
            FusionStrategy::MaxConfidence => {
                group.iter().map(|e| e.confidence).fold(0.0_f64, f64::max)
            }
            FusionStrategy::WeightedAverage { weights } => {
                let mut total_weight = 0.0;
                let mut weighted_sum = 0.0;
                for e in &group {
                    let w = weights.get(&e.detection_method).copied().unwrap_or(1.0);
                    weighted_sum += e.confidence * w;
                    total_weight += w;
                }
                if total_weight > 0.0 {
                    weighted_sum / total_weight
                } else {
                    0.0
                }
            }
            FusionStrategy::NoisyOr => {
                // P = 1 − ∏(1 − pᵢ)
                let product: f64 = group.iter().map(|e| 1.0 - e.confidence).product();
                1.0 - product
            }
        };

        // Use the first entity as the base and update confidence/method.
        let mut result = group.into_iter().next().unwrap();
        result.confidence = fused_confidence;
        result.detection_method = DetectionMethod::Composite;
        result
    }
}

impl Operation for Ensemble {
    type Input = ParallelContext<Vec<Entity>>;
    type Output = ParallelContext<Vec<Entity>>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.fuse(data)).await
    }
}

/// Check whether two optional locations overlap.
fn locations_overlap(a: &Option<Location>, b: &Option<Location>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(Location::Text(a_loc)), Some(Location::Text(b_loc))) => a_loc.overlaps(b_loc),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{EntityCategory, EntityKind, TextLocation};

    use super::*;

    fn text_entity(
        value: &str,
        method: DetectionMethod,
        confidence: f64,
        start: usize,
        end: usize,
    ) -> Entity {
        Entity::new(
            EntityCategory::Pii,
            EntityKind::PersonName,
            value,
            method,
            confidence,
        )
        .with_location(
            TextLocation {
                start_offset: start,
                end_offset: end,
                ..Default::default()
            }
            .into(),
        )
    }

    #[test]
    fn max_confidence_strategy() {
        let merge = Ensemble::new(FusionStrategy::MaxConfidence);
        let entities = vec![
            text_entity("John", DetectionMethod::Regex, 0.7, 0, 4),
            text_entity("John", DetectionMethod::Ner, 0.85, 0, 4),
        ];
        let result = merge.merge(entities);
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.85).abs() < f64::EPSILON);
        assert_eq!(result[0].detection_method, DetectionMethod::Composite);
    }

    #[test]
    fn noisy_or_strategy() {
        let merge = Ensemble::new(FusionStrategy::NoisyOr);
        let entities = vec![
            text_entity("John", DetectionMethod::Regex, 0.7, 0, 4),
            text_entity("John", DetectionMethod::Ner, 0.8, 0, 4),
        ];
        let result = merge.merge(entities);
        assert_eq!(result.len(), 1);
        // P = 1 − (1 − 0.7)(1 − 0.8) = 1 − (0.3)(0.2) = 0.94
        assert!((result[0].confidence - 0.94).abs() < 0.001);
    }

    #[test]
    fn weighted_average_strategy() {
        let mut weights = HashMap::new();
        weights.insert(DetectionMethod::Regex, 1.0);
        weights.insert(DetectionMethod::Ner, 2.0);

        let merge = Ensemble::new(FusionStrategy::WeightedAverage { weights });
        let entities = vec![
            text_entity("John", DetectionMethod::Regex, 0.6, 0, 4),
            text_entity("John", DetectionMethod::Ner, 0.9, 0, 4),
        ];
        let result = merge.merge(entities);
        assert_eq!(result.len(), 1);
        // (0.6 * 1.0 + 0.9 * 2.0) / (1.0 + 2.0) = 2.4 / 3.0 = 0.8
        assert!((result[0].confidence - 0.8).abs() < 0.001);
    }

    #[test]
    fn non_overlapping_not_merged() {
        let merge = Ensemble::new(FusionStrategy::NoisyOr);
        let entities = vec![
            text_entity("John", DetectionMethod::Regex, 0.7, 0, 4),
            text_entity("John", DetectionMethod::Ner, 0.8, 10, 14),
        ];
        let result = merge.merge(entities);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn single_entity_unchanged() {
        let merge = Ensemble::new(FusionStrategy::NoisyOr);
        let entities = vec![text_entity("John", DetectionMethod::Regex, 0.7, 0, 4)];
        let result = merge.merge(entities);
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.7).abs() < f64::EPSILON);
        assert_eq!(result[0].detection_method, DetectionMethod::Regex);
    }

    #[test]
    fn empty_input() {
        let merge = Ensemble::new(FusionStrategy::MaxConfidence);
        let result = merge.merge(Vec::new());
        assert!(result.is_empty());
    }
}
