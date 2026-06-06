//! [`FilterLayer`]: drop entities outside the allowed kinds or below
//! the confidence floor.
//!
//! Runs after calibration, before group/fuse. Dropped entities are
//! returned so a forthcoming drop-reason telemetry pass (#182) can
//! attribute them.

use async_trait::async_trait;
use nvisy_core::entity::{Entity, EntityKind};
use nvisy_core::extraction::TextAt;
use nvisy_core::modality::Modality;
use nvisy_core::primitive::ConfidenceThreshold;

use super::layer::{Layer, LayerContext};

/// Per-call filtering knobs applied during deduplication.
///
/// Empty knobs are no-ops — a `FilterParams` with all fields `None`
/// leaves every entity in place.
#[derive(Debug, Clone, Default)]
pub struct FilterParams {
    /// Drop entities whose `entity_kind` is outside this set.
    /// `None` keeps every kind.
    pub allowed_kinds: Option<Vec<EntityKind>>,
    /// Drop entities whose calibrated `confidence` is below this
    /// floor. `None` keeps every confidence level.
    pub confidence_threshold: Option<ConfidenceThreshold>,
}

impl FilterParams {
    /// Whether `entity` clears every configured filter knob.
    pub fn passes<M: Modality>(&self, entity: &Entity<M>) -> bool {
        if let Some(ref kinds) = self.allowed_kinds
            && !kinds.contains(&entity.entity_kind)
        {
            return false;
        }
        if let Some(threshold) = self.confidence_threshold
            && !threshold.admits(entity.confidence)
        {
            return false;
        }
        true
    }
}

/// [`Layer`] that drops entities not passing [`FilterParams`].
///
/// Returns the dropped entities from [`Layer::apply`].
pub struct FilterLayer {
    params: FilterParams,
}

impl FilterLayer {
    /// Construct a filter layer from a [`FilterParams`].
    pub fn new(params: FilterParams) -> Self {
        Self { params }
    }
}

#[async_trait]
impl<M: Modality, R: TextAt<M> + ?Sized> Layer<M, R> for FilterLayer {
    async fn apply(
        &self,
        entities: &mut Vec<Entity<M>>,
        _ctx: &LayerContext<'_, M, R>,
    ) -> Vec<Entity<M>> {
        let mut dropped = Vec::new();
        entities.retain(|e| {
            let keep = self.params.passes(e);
            if !keep {
                dropped.push(e.clone());
            }
            keep
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

    fn ent(kind: EntityKind, conf: f64) -> Entity<Text> {
        Entity::test_builder(0, 4)
            .with_entity_kind(kind)
            .with_confidence(Confidence::new(conf).expect("in range"))
            .test_build()
    }

    async fn apply<M: Modality>(
        params: FilterParams,
        entities: &mut Vec<Entity<M>>,
    ) -> Vec<Entity<M>> {
        let resolver = test_resolver::<M>();
        let ctx = LayerContext::new(&*resolver);
        let layer = FilterLayer::new(params);
        layer.apply(entities, &ctx).await
    }

    #[tokio::test]
    async fn default_params_keep_everything() {
        let mut entities: Vec<Entity<Text>> = vec![
            ent(EntityKind::PersonName, 0.9),
            ent(EntityKind::EmailAddress, 0.4),
        ];
        let dropped = apply(FilterParams::default(), &mut entities).await;
        assert_eq!(entities.len(), 2);
        assert!(dropped.is_empty());
    }

    #[tokio::test]
    async fn allowed_kinds_drops_outsiders() {
        let mut entities: Vec<Entity<Text>> = vec![
            ent(EntityKind::PersonName, 0.9),
            ent(EntityKind::EmailAddress, 0.9),
        ];
        let params = FilterParams {
            allowed_kinds: Some(vec![EntityKind::PersonName]),
            ..Default::default()
        };
        let dropped = apply(params, &mut entities).await;
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_kind, EntityKind::PersonName);
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].entity_kind, EntityKind::EmailAddress);
    }

    #[tokio::test]
    async fn confidence_threshold_drops_below() {
        let mut entities: Vec<Entity<Text>> = vec![
            ent(EntityKind::PersonName, 0.95),
            ent(EntityKind::PersonName, 0.40),
        ];
        let params = FilterParams {
            confidence_threshold: Some(ConfidenceThreshold::clamped(0.5)),
            ..Default::default()
        };
        let dropped = apply(params, &mut entities).await;
        assert_eq!(entities.len(), 1);
        assert!(entities[0].confidence.get() >= 0.5);
        assert_eq!(dropped.len(), 1);
    }

    #[tokio::test]
    async fn kinds_and_threshold_compose() {
        let mut entities: Vec<Entity<Text>> = vec![
            ent(EntityKind::PersonName, 0.95),   // keep
            ent(EntityKind::PersonName, 0.40),   // drop: threshold
            ent(EntityKind::EmailAddress, 0.95), // drop: kind
        ];
        let params = FilterParams {
            allowed_kinds: Some(vec![EntityKind::PersonName]),
            confidence_threshold: Some(ConfidenceThreshold::clamped(0.5)),
        };
        let dropped = apply(params, &mut entities).await;
        assert_eq!(entities.len(), 1);
        assert_eq!(dropped.len(), 2);
    }
}
