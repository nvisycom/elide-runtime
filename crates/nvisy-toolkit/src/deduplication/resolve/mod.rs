//! [`ResolveConflictsLayer`]: drop the loser of each cross-kind span
//! overlap.
//!
//! When entities of different kinds overlap the same text span (e.g.
//! "555-1234" matches both `PhoneNumber` and `EmailAddress`), this
//! layer keeps only the winner per the configured
//! [`ConflictResolution`] strategy.

mod strategy;

use async_trait::async_trait;
use nvisy_core::ValueAt;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Modality, Overlap};

pub use self::strategy::ConflictResolution;
use super::layer::{Layer, LayerContext};
use super::span_size::SpanSize;

const TARGET: &str = "nvisy_document::deduplication::resolve";

/// [`Layer`] that resolves cross-kind span overlaps in place per the
/// given [`ConflictResolution`] strategy.
///
/// Returns the dropped (losing) entities from [`Layer::apply`].
pub struct ResolveConflictsLayer {
    strategy: ConflictResolution,
}

impl ResolveConflictsLayer {
    /// Construct a resolve layer from a strategy.
    pub fn new(strategy: ConflictResolution) -> Self {
        Self { strategy }
    }
}

#[async_trait]
impl<M, R> Layer<M, R> for ResolveConflictsLayer
where
    M: Modality,
    M::Location: Overlap + SpanSize,
    R: ValueAt<M> + ?Sized,
{
    async fn apply(
        &self,
        entities: &mut Vec<Entity<M>>,
        _ctx: &LayerContext<'_, M, R>,
    ) -> Vec<Entity<M>> {
        if entities.len() <= 1 {
            return Vec::new();
        }

        let len = entities.len();
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
                if entities[i].entity_kind == entities[j].entity_kind {
                    continue;
                }
                if !entities[i].location.overlaps(&entities[j].location) {
                    continue;
                }

                if self.strategy.keeps_first(&entities[i], &entities[j]) {
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
                strategy = ?self.strategy,
                "span conflicts resolved",
            );
        }

        let mut dropped = Vec::new();
        let mut idx = 0usize;
        entities.retain(|entity| {
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
    use nvisy_core::entity::{Entity, EntityKind};
    use nvisy_core::modality::Text;
    use nvisy_core::primitive::Confidence;

    use super::*;
    use crate::deduplication::test_resolver;

    fn conf(v: f64) -> Confidence {
        Confidence::new(v).expect("confidence in [0,1]")
    }

    async fn apply<M>(strategy: ConflictResolution, entities: &mut Vec<Entity<M>>) -> Vec<Entity<M>>
    where
        M: Modality,
        M::Location: Overlap + SpanSize,
    {
        let resolver = test_resolver::<M>();
        let ctx = LayerContext::new(&*resolver);
        let layer = ResolveConflictsLayer::new(strategy);
        layer.apply(entities, &ctx).await
    }

    #[tokio::test]
    async fn highest_confidence_keeps_winner() {
        let mut entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::PhoneNumber)
                .test_build(),
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::EmailAddress)
                .with_confidence(conf(0.8))
                .test_build(),
        ];
        let _ = apply(ConflictResolution::HighestConfidence, &mut entities).await;
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_kind, EntityKind::PhoneNumber);
    }

    #[tokio::test]
    async fn non_overlapping_not_resolved() {
        let mut entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::PhoneNumber)
                .test_build(),
            Entity::test_builder(20, 24)
                .with_entity_kind(EntityKind::EmailAddress)
                .with_confidence(conf(0.8))
                .test_build(),
        ];
        let _ = apply(ConflictResolution::HighestConfidence, &mut entities).await;
        assert_eq!(entities.len(), 2);
    }

    #[tokio::test]
    async fn same_kind_not_resolved() {
        let mut entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::PhoneNumber)
                .test_build(),
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::PhoneNumber)
                .with_confidence(conf(0.8))
                .test_build(),
        ];
        let _ = apply(ConflictResolution::HighestConfidence, &mut entities).await;
        assert_eq!(entities.len(), 2);
    }

    #[tokio::test]
    async fn longest_span_keeps_longer() {
        let mut entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 3)
                .with_entity_kind(EntityKind::PhoneNumber)
                .test_build(),
            Entity::test_builder(0, 8)
                .with_entity_kind(EntityKind::EmailAddress)
                .with_confidence(conf(0.7))
                .test_build(),
        ];
        let _ = apply(ConflictResolution::LongestSpan, &mut entities).await;
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_kind, EntityKind::EmailAddress);
    }
}
