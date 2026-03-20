//! Entity fusion operation.
//!
//! Runs at **phase 3**, after detection. Combines the deduplication pass
//! (merge exact duplicates) and the ensemble pass (fuse multi-detector
//! confidence scores) into a single operation.

use nvisy_core::{Error, Result};
use nvisy_ontology::entity::{Entities, Entity, Overlap, RefinementMethod};

use crate::graph::{Fusion as FusionCfg, FusionStrategy};
use crate::operation::context::ParallelContext;
use crate::operation::envelope::RefinedEntities;
use crate::operation::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::fusion";

/// Combined deduplication + ensemble fusion operation.
pub struct Fusion {
    deduplicate: bool,
    strategy: FusionStrategy,
}

impl Fusion {
    /// Create from graph config.
    pub fn new(cfg: &FusionCfg) -> Self {
        if cfg.confidence_calibration {
            tracing::warn!(target: TARGET, "confidence_calibration not yet implemented, skipping");
        }
        if cfg.contextual_adjustment {
            tracing::warn!(target: TARGET, "contextual_adjustment not yet implemented, skipping");
        }
        Self {
            deduplicate: cfg.entity_deduplication,
            strategy: cfg.strategy.clone(),
        }
    }

    pub(crate) async fn execute(&self, entities: Entities) -> Result<RefinedEntities> {
        if entities.is_empty() {
            return Ok(RefinedEntities(entities));
        }

        let before = entities.len();

        let entities = if self.deduplicate {
            Self::deduplicate(entities)
        } else {
            entities
        };

        let after_dedup = entities.len();
        let result = self.strategy.fuse(entities);

        tracing::debug!(
            target: TARGET,
            before,
            after_dedup,
            after_fusion = result.len(),
            "fusion complete",
        );

        Ok(RefinedEntities(result))
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
                    && existing.location.overlaps(&entity.location)
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
}

impl Operation for Fusion {
    type Input = ParallelContext<Entities>;
    type Output = ParallelContext<RefinedEntities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.execute(data)).await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use nvisy_ontology::entity::{EntityCategory, EntityKind, RecognitionMethod, TextLocation};

    use super::*;
    use crate::graph::FusionStrategy::*;

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
        let result = Fusion::deduplicate(entities);
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
        let result = Fusion::deduplicate(entities);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn max_confidence_strategy() {
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.7, 0, 4),
            text_entity("John", RecognitionMethod::Ner, 0.85, 0, 4),
        ]
        .into();
        let result = MaxConfidence.fuse(entities);
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
        let result = NoisyOr.fuse(entities);
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
        let result = WeightedAverage { weights }.fuse(entities);
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.8).abs() < 0.001);
    }

    #[tokio::test]
    async fn full_pipeline_dedup_then_fuse() {
        let fusion = Fusion {
            deduplicate: true,
            strategy: FusionStrategy::MaxConfidence,
        };
        let entities: Entities = vec![
            text_entity("John", RecognitionMethod::Regex, 0.7, 0, 4),
            text_entity("John", RecognitionMethod::Regex, 0.8, 0, 4),
            text_entity("John", RecognitionMethod::Ner, 0.85, 0, 4),
        ]
        .into();

        let result = fusion.execute(entities).await.unwrap();
        assert_eq!(result.0.len(), 1);
        assert!((result.0[0].confidence - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn empty_input() {
        let result = Fusion::deduplicate(Entities::new());
        assert!(result.is_empty());
    }
}
