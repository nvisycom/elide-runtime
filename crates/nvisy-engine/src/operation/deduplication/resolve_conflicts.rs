//! Cross-kind span conflict resolution.
//!
//! When entities of different kinds overlap the same text span (e.g.
//! "555-1234" matches both `PhoneNumber` and `EmailAddress`), this
//! module resolves the conflict by keeping only the winner based on
//! the configured [`ConflictResolution`] strategy.

use std::cmp::Ordering;

use nvisy_ontology::entity::{Entities, Entity, Overlap};
use nvisy_ontology::workflow::ConflictResolution;

use super::span_size::SpanSize;

const TARGET: &str = "nvisy_engine::op::deduplication::conflict";

/// Extension trait for [`ConflictResolution`].
pub(super) trait ConflictResolutionExt {
    /// Resolve cross-kind span conflicts, returning the filtered list.
    ///
    /// For each pair of different-kind entities that overlap, the loser
    /// is removed according to the resolution strategy.
    fn resolve(&self, entities: Entities) -> Entities;

    /// Given two overlapping, different-kind entities, return `true` if
    /// `a` wins over `b`.
    fn should_keep_first(&self, a: &Entity, b: &Entity) -> bool;
}

impl ConflictResolutionExt for ConflictResolution {
    fn resolve(&self, entities: Entities) -> Entities {
        if entities.len() <= 1 {
            return entities;
        }

        let len = entities.len();
        let inner = entities.into_inner();
        let mut losers = vec![false; len];
        let mut resolved = 0usize;

        // O(n²) pairwise check — acceptable because entity counts are
        // typically small (tens to low hundreds) after deduplication.
        for i in 0..len {
            if losers[i] {
                continue;
            }
            for j in (i + 1)..len {
                if losers[j] {
                    continue;
                }
                // Only resolve conflicts between *different* kinds —
                // same-kind overlaps are handled by the grouping/fusion pass.
                if inner[i].entity_kind == inner[j].entity_kind {
                    continue;
                }
                if !inner[i].location.overlaps(&inner[j].location) {
                    continue;
                }

                if self.should_keep_first(&inner[i], &inner[j]) {
                    losers[j] = true;
                } else {
                    losers[i] = true;
                }
                resolved += 1;
            }
        }

        if resolved > 0 {
            tracing::debug!(
                target: TARGET,
                resolved,
                strategy = ?self,
                "span conflicts resolved",
            );
        }

        inner
            .into_iter()
            .enumerate()
            .filter(|(i, _)| !losers[*i])
            .map(|(_, e)| e)
            .collect()
    }

    fn should_keep_first(&self, a: &Entity, b: &Entity) -> bool {
        match self {
            Self::HighestConfidence => a.confidence.get() >= b.confidence.get(),
            Self::HighestSensitivity => match (a.sensitivity, b.sensitivity) {
                (Some(sa), Some(sb)) => sa >= sb,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => a.confidence.get() >= b.confidence.get(),
            },
            Self::LongestSpan => {
                a.location.span_cmp(&b.location).unwrap_or(Ordering::Equal) != Ordering::Less
            }
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{Entity, EntityKind};
    use nvisy_ontology::primitive::Confidence;

    use super::*;

    fn conf(v: f64) -> Confidence {
        Confidence::new(v).expect("confidence in [0,1]")
    }

    #[test]
    fn highest_confidence_keeps_winner() {
        let entities: Entities = vec![
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::PhoneNumber)
                .test_build(),
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::EmailAddress)
                .with_confidence(conf(0.8))
                .test_build(),
        ]
        .into();
        let result = ConflictResolution::HighestConfidence.resolve(entities);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].entity_kind, EntityKind::PhoneNumber);
    }

    #[test]
    fn non_overlapping_not_resolved() {
        let entities: Entities = vec![
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::PhoneNumber)
                .test_build(),
            Entity::test_builder(20, 24)
                .with_entity_kind(EntityKind::EmailAddress)
                .with_confidence(conf(0.8))
                .test_build(),
        ]
        .into();
        let result = ConflictResolution::HighestConfidence.resolve(entities);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn same_kind_not_resolved() {
        let entities: Entities = vec![
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::PhoneNumber)
                .test_build(),
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::PhoneNumber)
                .with_confidence(conf(0.8))
                .test_build(),
        ]
        .into();
        let result = ConflictResolution::HighestConfidence.resolve(entities);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn longest_span_keeps_longer() {
        let entities: Entities = vec![
            Entity::test_builder(0, 3)
                .with_entity_kind(EntityKind::PhoneNumber)
                .test_build(),
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::EmailAddress)
                .with_confidence(conf(0.7))
                .test_build(),
        ]
        .into();
        let result = ConflictResolution::LongestSpan.resolve(entities);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].entity_kind, EntityKind::EmailAddress);
    }
}
