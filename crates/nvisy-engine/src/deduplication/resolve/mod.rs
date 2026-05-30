//! Cross-kind span conflict resolution.
//!
//! When entities of different kinds overlap the same text span (e.g.
//! "555-1234" matches both `PhoneNumber` and `EmailAddress`), this
//! module resolves the conflict by keeping only the winner based on
//! the configured [`ConflictResolution`] strategy.

mod strategy;

use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::{Modality, Overlap};

pub use self::strategy::ConflictResolution;
use crate::deduplication::span_size::SpanSize;

const TARGET: &str = "nvisy_engine::op::deduplication::conflict";

/// Extension trait: resolve cross-kind span overlaps in place per the
/// given [`ConflictResolution`] strategy.
pub(super) trait ResolveConflicts<M: Modality> {
    /// Drop the loser of each cross-kind overlap; returns the
    /// dropped entities for downstream telemetry.
    fn resolve_conflicts(&mut self, strategy: &ConflictResolution) -> Vec<Entity<M>>;
}

impl<M> ResolveConflicts<M> for Vec<Entity<M>>
where
    M: Modality + Overlap + SpanSize,
{
    fn resolve_conflicts(&mut self, strategy: &ConflictResolution) -> Vec<Entity<M>> {
        if self.len() <= 1 {
            return Vec::new();
        }

        let len = self.len();
        let mut losers = vec![false; len];
        let mut resolved = 0usize;

        for i in 0..len {
            if losers[i] {
                continue;
            }
            for j in (i + 1)..len {
                if losers[j] {
                    continue;
                }
                if self[i].entity_kind == self[j].entity_kind {
                    continue;
                }
                if !self[i].location.overlaps(&self[j].location) {
                    continue;
                }

                if strategy.keeps_first(&self[i], &self[j]) {
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
                strategy = ?strategy,
                "span conflicts resolved",
            );
        }

        let mut dropped = Vec::new();
        let mut idx = 0usize;
        self.retain(|entity| {
            let lost = losers[idx];
            idx += 1;
            if lost {
                dropped.push(entity.clone());
            }
            !lost
        });
        dropped
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{Entity, EntityKind};
    use nvisy_ontology::modality::Text;
    use nvisy_ontology::primitive::Confidence;

    use super::*;

    fn conf(v: f64) -> Confidence {
        Confidence::new(v).expect("confidence in [0,1]")
    }

    #[test]
    fn highest_confidence_keeps_winner() {
        let mut entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::PhoneNumber)
                .test_build(),
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::EmailAddress)
                .with_confidence(conf(0.8))
                .test_build(),
        ];
        let _ = entities.resolve_conflicts(&ConflictResolution::HighestConfidence);
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_kind, EntityKind::PhoneNumber);
    }

    #[test]
    fn non_overlapping_not_resolved() {
        let mut entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::PhoneNumber)
                .test_build(),
            Entity::test_builder(20, 24)
                .with_entity_kind(EntityKind::EmailAddress)
                .with_confidence(conf(0.8))
                .test_build(),
        ];
        let _ = entities.resolve_conflicts(&ConflictResolution::HighestConfidence);
        assert_eq!(entities.len(), 2);
    }

    #[test]
    fn same_kind_not_resolved() {
        let mut entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::PhoneNumber)
                .test_build(),
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::PhoneNumber)
                .with_confidence(conf(0.8))
                .test_build(),
        ];
        let _ = entities.resolve_conflicts(&ConflictResolution::HighestConfidence);
        assert_eq!(entities.len(), 2);
    }

    #[test]
    fn longest_span_keeps_longer() {
        let mut entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 3)
                .with_entity_kind(EntityKind::PhoneNumber)
                .test_build(),
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::EmailAddress)
                .with_confidence(conf(0.7))
                .test_build(),
        ];
        let _ = entities.resolve_conflicts(&ConflictResolution::LongestSpan);
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_kind, EntityKind::EmailAddress);
    }
}
