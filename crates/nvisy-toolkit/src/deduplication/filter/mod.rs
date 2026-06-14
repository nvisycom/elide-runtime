//! [`FilterLayer`]: drop entities outside the allowed labels or
//! below the confidence floor.
//!
//! Dropped entities are returned from [`Layer::apply`] so the
//! pipeline can attribute them in its drop-reason roll-up.
//!
//! [`Layer::apply`]: super::layer::Layer::apply

use nvisy_core::entity::{Entity, EntityLabelRef};
use nvisy_core::extraction::TextAt;
use nvisy_core::modality::Modality;
use nvisy_core::primitive::ConfidenceThreshold;

use super::layer::{Layer, LayerContext};

/// [`Layer`] that drops entities outside the allowed labels or
/// below the confidence floor. Returns the dropped entities from
/// [`Layer::apply`].
///
/// Construct empty with [`FilterLayer::new`] (default = pass
/// everything) and configure via [`with_allowed_labels`] /
/// [`with_confidence_threshold`].
///
/// [`with_allowed_labels`]: Self::with_allowed_labels
/// [`with_confidence_threshold`]: Self::with_confidence_threshold
#[derive(Debug, Clone, Default)]
pub struct FilterLayer {
    allowed_labels: Option<Vec<EntityLabelRef>>,
    confidence_threshold: Option<ConfidenceThreshold>,
}

impl FilterLayer {
    /// Empty filter: keeps every entity.
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop entities whose `label` is outside this set.
    /// `None` keeps every label (same as not calling this).
    #[must_use]
    pub fn with_allowed_labels(mut self, labels: Option<Vec<EntityLabelRef>>) -> Self {
        self.allowed_labels = labels;
        self
    }

    /// Drop entities whose calibrated `confidence` is below this
    /// floor. `None` keeps every confidence level (same as not
    /// calling this).
    #[must_use]
    pub fn with_confidence_threshold(mut self, threshold: Option<ConfidenceThreshold>) -> Self {
        self.confidence_threshold = threshold;
        self
    }

    /// Whether `entity` clears every configured filter knob.
    pub fn passes<M: Modality>(&self, entity: &Entity<M>) -> bool {
        if let Some(ref labels) = self.allowed_labels
            && !labels.contains(&entity.label)
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

#[async_trait::async_trait]
impl<M: Modality, R: TextAt<M> + ?Sized> Layer<M, R> for FilterLayer {
    async fn apply(
        &self,
        entities: &mut Vec<Entity<M>>,
        _ctx: &LayerContext<'_, M, R>,
    ) -> Vec<Entity<M>> {
        let mut dropped = Vec::new();
        entities.retain(|e| {
            let keep = self.passes(e);
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
    use nvisy_core::entity::{Entity, EntityLabelRef, builtins};
    use nvisy_core::modality::Text;
    use nvisy_core::primitive::Confidence;

    use super::*;
    use crate::deduplication::test_resolver;

    fn ent(label: EntityLabelRef, conf: f64) -> Entity<Text> {
        Entity::test_builder(0, 4)
            .with_label(label)
            .with_confidence(Confidence::new(conf).expect("in range"))
            .test_build()
    }

    async fn apply<M: Modality>(
        layer: FilterLayer,
        entities: &mut Vec<Entity<M>>,
    ) -> Vec<Entity<M>> {
        let resolver = test_resolver::<M>();
        let ctx = LayerContext::new(&*resolver);
        layer.apply(entities, &ctx).await
    }

    #[tokio::test]
    async fn default_layer_keeps_everything() {
        let mut entities: Vec<Entity<Text>> = vec![
            ent(builtins::PERSON_NAME.label_ref(), 0.9),
            ent(builtins::EMAIL_ADDRESS.label_ref(), 0.4),
        ];
        let dropped = apply(FilterLayer::new(), &mut entities).await;
        assert_eq!(entities.len(), 2);
        assert!(dropped.is_empty());
    }

    #[tokio::test]
    async fn allowed_labels_drops_outsiders() {
        let mut entities: Vec<Entity<Text>> = vec![
            ent(builtins::PERSON_NAME.label_ref(), 0.9),
            ent(builtins::EMAIL_ADDRESS.label_ref(), 0.9),
        ];
        let layer =
            FilterLayer::new().with_allowed_labels(Some(vec![builtins::PERSON_NAME.label_ref()]));
        let dropped = apply(layer, &mut entities).await;
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].label, builtins::PERSON_NAME.label_ref());
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].label, builtins::EMAIL_ADDRESS.label_ref());
    }

    #[tokio::test]
    async fn confidence_threshold_drops_below() {
        let mut entities: Vec<Entity<Text>> = vec![
            ent(builtins::PERSON_NAME.label_ref(), 0.95),
            ent(builtins::PERSON_NAME.label_ref(), 0.40),
        ];
        let layer =
            FilterLayer::new().with_confidence_threshold(Some(ConfidenceThreshold::clamped(0.5)));
        let dropped = apply(layer, &mut entities).await;
        assert_eq!(entities.len(), 1);
        assert!(entities[0].confidence.get() >= 0.5);
        assert_eq!(dropped.len(), 1);
    }

    #[tokio::test]
    async fn labels_and_threshold_compose() {
        let mut entities: Vec<Entity<Text>> = vec![
            ent(builtins::PERSON_NAME.label_ref(), 0.95),   // keep
            ent(builtins::PERSON_NAME.label_ref(), 0.40),   // drop: threshold
            ent(builtins::EMAIL_ADDRESS.label_ref(), 0.95), // drop: label
        ];
        let layer = FilterLayer::new()
            .with_allowed_labels(Some(vec![builtins::PERSON_NAME.label_ref()]))
            .with_confidence_threshold(Some(ConfidenceThreshold::clamped(0.5)));
        let dropped = apply(layer, &mut entities).await;
        assert_eq!(entities.len(), 1);
        assert_eq!(dropped.len(), 2);
    }
}
