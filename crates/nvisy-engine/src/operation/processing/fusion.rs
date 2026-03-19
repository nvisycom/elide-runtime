//! Entity fusion: deduplication and ensemble confidence merging.
//!
//! Combines the deduplication pass (merge exact duplicates) and
//! the ensemble pass (fuse multi-detector confidence scores) into
//! a single operation.

use std::collections::HashMap;

use nvisy_core::Result;
use nvisy_ontology::entity::{
    Entities, Entity, Location, RecognitionMethod, RefinementMethod,
};

use crate::operation::envelope::RefinedEntities;
use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::fusion";

/// Strategy for combining confidence scores from multiple detectors.
#[derive(Debug, Clone)]
pub enum FusionStrategy {
    /// Take the maximum confidence across all detectors.
    MaxConfidence,
    /// Weighted average by recognition method.
    WeightedAverage {
        weights: HashMap<RecognitionMethod, f64>,
    },
    /// Noisy-OR: `P = 1 − ∏(1 − pᵢ)` for independent detectors.
    NoisyOr,
}

/// Configuration for the fusion operation.
#[derive(Debug, Clone)]
pub struct FusionParams {
    /// Run deduplication before ensemble fusion.
    pub deduplicate: bool,
    /// Confidence combination strategy.
    pub strategy: FusionStrategy,
}

impl Default for FusionParams {
    fn default() -> Self {
        Self {
            deduplicate: true,
            strategy: FusionStrategy::MaxConfidence,
        }
    }
}

/// Combined deduplication + ensemble fusion operation.
pub struct Fusion {
    params: FusionParams,
}

impl Fusion {
    pub fn new(params: FusionParams) -> Self {
        Self { params }
    }

    async fn execute(&self, entities: Entities) -> Result<RefinedEntities> {
        if entities.is_empty() {
            return Ok(RefinedEntities(entities));
        }

        let before = entities.len();

        let entities = if self.params.deduplicate {
            deduplicate(entities)
        } else {
            entities
        };

        let after_dedup = entities.len();
        let result = ensemble(&self.params.strategy, entities);

        tracing::debug!(
            target: TARGET,
            before,
            after_dedup,
            after_fusion = result.len(),
            "fusion complete",
        );

        Ok(RefinedEntities(result))
    }
}

impl Operation for Fusion {
    type Input = ParallelContext<Entities>;
    type Output = ParallelContext<RefinedEntities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.execute(data)).await
    }
}

/// Check whether two optional locations overlap.
///
/// Two entities with no location are considered distinct — merging
/// locationless entities risks combining different occurrences.
pub(crate) fn locations_overlap(a: &Option<Location>, b: &Option<Location>) -> bool {
    match (a, b) {
        (None, None) => false,
        (Some(Location::Text(a_loc)), Some(Location::Text(b_loc))) => a_loc.overlaps(b_loc),
        _ => false,
    }
}

fn deduplicate(entities: Entities) -> Entities {
    if entities.len() <= 1 {
        return entities;
    }

    let mut result: Vec<Entity> = Vec::new();

    for entity in entities {
        let merged = result.iter_mut().find(|existing| {
            existing.entity_kind == entity.entity_kind
                && existing.value == entity.value
                && locations_overlap(&existing.location, &entity.location)
        });

        match merged {
            Some(existing) => {
                if entity.confidence > existing.confidence {
                    existing.confidence = entity.confidence;
                }
                for m in entity.recognition_methods {
                    if !existing.recognition_methods.contains(&m) {
                        existing.recognition_methods.push(m);
                    }
                }
                if !existing
                    .refinement_methods
                    .contains(&RefinementMethod::Deduplication)
                {
                    existing
                        .refinement_methods
                        .push(RefinementMethod::Deduplication);
                }
            }
            None => {
                result.push(entity);
            }
        }
    }

    result.into()
}

fn ensemble(strategy: &FusionStrategy, entities: Entities) -> Entities {
    if entities.len() <= 1 {
        return entities;
    }

    let mut groups: Vec<Vec<Entity>> = Vec::new();

    for entity in entities {
        let group = groups.iter_mut().find(|group| {
            let rep = &group[0];
            rep.entity_kind == entity.entity_kind
                && rep.value == entity.value
                && locations_overlap(&rep.location, &entity.location)
        });

        match group {
            Some(g) => g.push(entity),
            None => groups.push(vec![entity]),
        }
    }

    groups
        .into_iter()
        .map(|group| fuse_group(strategy, group))
        .collect()
}

fn fuse_group(strategy: &FusionStrategy, group: Vec<Entity>) -> Entity {
    debug_assert!(!group.is_empty());

    if group.len() == 1 {
        return group.into_iter().next().unwrap();
    }

    let fused_confidence = match strategy {
        FusionStrategy::MaxConfidence => {
            group.iter().map(|e| e.confidence).fold(0.0_f64, f64::max)
        }
        FusionStrategy::WeightedAverage { weights } => {
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
        FusionStrategy::NoisyOr => {
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

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{EntityCategory, EntityKind, TextLocation};

    use super::*;

    fn text_entity(
        value: &str,
        method: RecognitionMethod,
        confidence: f64,
        start: usize,
        end: usize,
    ) -> Entity {
        Entity::new(
            EntityCategory::PersonalIdentity,
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
    fn dedup_merges_duplicates() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.8, 0, 4),
            text_entity("John", RecognitionMethod::Regex, 0.9, 0, 4),
        ]
        .into();
        let result = deduplicate(entities);
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn dedup_preserves_non_overlapping() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.8, 0, 4),
            text_entity("John", RecognitionMethod::Regex, 0.9, 10, 14),
        ]
        .into();
        let result = deduplicate(entities);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn max_confidence_strategy() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.7, 0, 4),
            text_entity("John", RecognitionMethod::Ner, 0.85, 0, 4),
        ]
        .into();
        let result = ensemble(&FusionStrategy::MaxConfidence, entities);
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn noisy_or_strategy() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.7, 0, 4),
            text_entity("John", RecognitionMethod::Ner, 0.8, 0, 4),
        ]
        .into();
        let result = ensemble(&FusionStrategy::NoisyOr, entities);
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.94).abs() < 0.001);
    }

    #[test]
    fn weighted_average_strategy() {
        let mut weights = HashMap::new();
        weights.insert(RecognitionMethod::Regex, 1.0);
        weights.insert(RecognitionMethod::Ner, 2.0);

        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.6, 0, 4),
            text_entity("John", RecognitionMethod::Ner, 0.9, 0, 4),
        ]
        .into();
        let result = ensemble(&FusionStrategy::WeightedAverage { weights }, entities);
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.8).abs() < 0.001);
    }

    #[test]
    fn full_pipeline_dedup_then_fuse() {
        let fusion = Fusion::new(FusionParams::default());
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.7, 0, 4),
            text_entity("John", RecognitionMethod::Regex, 0.8, 0, 4),
            text_entity("John", RecognitionMethod::Ner, 0.85, 0, 4),
        ]
        .into();

        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(fusion.execute(entities))
            .unwrap();
        assert_eq!(result.0.len(), 1);
        assert!((result.0[0].confidence - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn empty_input() {
        let result = deduplicate(Entities::new());
        assert!(result.is_empty());
    }
}
