//! [`Filter`]: per-call entity filtering applied during
//! deduplication, after calibrate and before group/fuse.
//!
//! The deduplicator builds a [`FilterParams`] from the operator's
//! [`DetectionContext`] (with the engine's
//! [`Deduplicator::confidence_threshold`] as fallback) and calls
//! [`Entities::filter`] in-place. Dropped entities are returned so a
//! forthcoming drop-reason telemetry pass (#182) can attribute them.
//!
//! [`DetectionContext`]: crate::detection::DetectionContext
//! [`Deduplicator::confidence_threshold`]: super::Deduplicator

use nvisy_ontology::entity::{Entities, Entity, EntityKind};
use nvisy_ontology::modality::AnyModality;

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
    pub confidence_threshold: Option<f64>,
}

/// Extension trait on [`Entities`]: drop entries that don't pass
/// `params`, returning the dropped vec.
pub(crate) trait Filter {
    /// Remove entities not passing `params` in-place; return the
    /// removed entities for downstream telemetry (#182).
    fn filter(&mut self, params: &FilterParams) -> Entities<AnyModality>;
}

impl Filter for Entities<AnyModality> {
    fn filter(&mut self, params: &FilterParams) -> Entities<AnyModality> {
        let mut dropped = Entities::new();
        self.0.retain(|e| {
            let keep = passes(e, params);
            if !keep {
                dropped.push(e.clone());
            }
            keep
        });
        dropped
    }
}

fn passes(entity: &Entity<AnyModality>, params: &FilterParams) -> bool {
    if let Some(ref kinds) = params.allowed_kinds
        && !kinds.contains(&entity.entity_kind)
    {
        return false;
    }
    if let Some(threshold) = params.confidence_threshold
        && entity.confidence.get() < threshold
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{Entity, EntityKind};
    use nvisy_ontology::primitive::Confidence;

    use super::*;

    fn ent(kind: EntityKind, conf: f64) -> Entity {
        Entity::test_builder(0, 4)
            .with_entity_kind(kind)
            .with_confidence(Confidence::new(conf).expect("in range"))
            .test_build()
    }

    #[test]
    fn default_params_keep_everything() {
        let mut entities = Entities::from(vec![
            ent(EntityKind::PersonName, 0.9),
            ent(EntityKind::EmailAddress, 0.4),
        ]);
        let dropped = entities.filter(&FilterParams::default());
        assert_eq!(entities.len(), 2);
        assert!(dropped.is_empty());
    }

    #[test]
    fn allowed_kinds_drops_outsiders() {
        let mut entities = Entities::from(vec![
            ent(EntityKind::PersonName, 0.9),
            ent(EntityKind::EmailAddress, 0.9),
        ]);
        let params = FilterParams {
            allowed_kinds: Some(vec![EntityKind::PersonName]),
            ..Default::default()
        };
        let dropped = entities.filter(&params);
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_kind, EntityKind::PersonName);
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].entity_kind, EntityKind::EmailAddress);
    }

    #[test]
    fn confidence_threshold_drops_below() {
        let mut entities = Entities::from(vec![
            ent(EntityKind::PersonName, 0.95),
            ent(EntityKind::PersonName, 0.40),
        ]);
        let params = FilterParams {
            confidence_threshold: Some(0.5),
            ..Default::default()
        };
        let dropped = entities.filter(&params);
        assert_eq!(entities.len(), 1);
        assert!(entities[0].confidence.get() >= 0.5);
        assert_eq!(dropped.len(), 1);
    }

    #[test]
    fn kinds_and_threshold_compose() {
        let mut entities = Entities::from(vec![
            ent(EntityKind::PersonName, 0.95),   // keep
            ent(EntityKind::PersonName, 0.40),   // drop: threshold
            ent(EntityKind::EmailAddress, 0.95), // drop: kind
        ]);
        let params = FilterParams {
            allowed_kinds: Some(vec![EntityKind::PersonName]),
            confidence_threshold: Some(0.5),
        };
        let dropped = entities.filter(&params);
        assert_eq!(entities.len(), 1);
        assert_eq!(dropped.len(), 2);
    }
}
