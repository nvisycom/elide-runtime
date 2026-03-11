//! Cross-layer entity deduplication.
//!
//! Merges entities that share the same `entity_kind`, `value`, and
//! overlapping location into a single entity with the highest
//! confidence and `DetectionMethod::Composite` when methods differ.

use nvisy_core::Result;
use nvisy_ontology::entity::{DetectionMethod, Entities, Entity, Location};

use crate::operation::envelope::RefinedEntities;
use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::deduplication";

/// Deduplicates a list of entities by merging duplicates.
///
/// Two entities are considered duplicates when they have the same
/// `entity_kind` and `value` and their locations overlap.
///
/// When merging:
/// - The highest confidence score is kept.
/// - If the detection methods differ, the merged entity uses
///   `DetectionMethod::Composite`.
pub struct Deduplication;

impl Deduplication {
    async fn deduplicate(&self, entities: Entities) -> Result<RefinedEntities> {
        let before = entities.len();
        let result = Self::execute(entities);
        tracing::debug!(target: TARGET, before, after = result.len(), "deduplicated entities");
        Ok(RefinedEntities(result))
    }

    /// Deduplicate and merge overlapping entities.
    pub fn execute(entities: Entities) -> Entities {
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
                    if existing.detection_method != entity.detection_method {
                        existing.detection_method = DetectionMethod::Composite;
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

impl Operation for Deduplication {
    type Input = ParallelContext<Entities>;
    type Output = ParallelContext<RefinedEntities>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.deduplicate(data)).await
    }
}

/// Check whether two optional locations overlap.
///
/// Currently supports overlap detection for text locations.
/// Other modalities are considered overlapping only if both are `None`.
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
    fn duplicates_merged_same_method() {
        let entities: Entities = vec![
            text_entity("John", DetectionMethod::Regex, 0.8, 0, 4),
            text_entity("John", DetectionMethod::Regex, 0.9, 0, 4),
        ]
        .into();
        let result = Deduplication::execute(entities);
        assert_eq!(result.len(), 1);
        assert!((result[0].confidence - 0.9).abs() < f64::EPSILON);
        assert_eq!(result[0].detection_method, DetectionMethod::Regex);
    }

    #[test]
    fn different_methods_become_composite() {
        let entities: Entities = vec![
            text_entity("John", DetectionMethod::Regex, 0.8, 0, 4),
            text_entity("John", DetectionMethod::Ner, 0.85, 0, 4),
        ]
        .into();
        let result = Deduplication::execute(entities);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].detection_method, DetectionMethod::Composite);
        assert!((result[0].confidence - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn non_overlapping_preserved() {
        let entities: Entities = vec![
            text_entity("John", DetectionMethod::Regex, 0.8, 0, 4),
            text_entity("John", DetectionMethod::Regex, 0.9, 10, 14),
        ]
        .into();
        let result = Deduplication::execute(entities);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn different_values_not_merged() {
        let entities: Entities = vec![
            text_entity("John", DetectionMethod::Regex, 0.8, 0, 4),
            text_entity("Jane", DetectionMethod::Regex, 0.9, 0, 4),
        ]
        .into();
        let result = Deduplication::execute(entities);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn empty_input() {
        let result = Deduplication::execute(Entities::new());
        assert!(result.is_empty());
    }

    #[test]
    fn single_entity_unchanged() {
        let entities: Entities =
            vec![text_entity("John", DetectionMethod::Regex, 0.8, 0, 4)].into();
        let result = Deduplication::execute(entities);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn overlapping_ranges_merge() {
        // Partially overlapping: 0..6 and 3..9.
        let entities: Entities = vec![
            text_entity("John Doe", DetectionMethod::Regex, 0.7, 0, 6),
            text_entity("John Doe", DetectionMethod::Ner, 0.9, 3, 9),
        ]
        .into();
        let result = Deduplication::execute(entities);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].detection_method, DetectionMethod::Composite);
    }
}
