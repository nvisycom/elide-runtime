//! Per-recognizer confidence calibration.
//!
//! Scales entity confidence scores using per-recognizer multipliers
//! before deduplication. This compensates for score distribution
//! differences between detectors: regex always returns 1.0 while NER
//! returns 0.3–0.9, so a multiplier of 0.8 on `pattern` brings them
//! into alignment.
//!
//! Keys are the recognizer source names stamped onto the entity's
//! [`TrailStep::recognition`]
//! step — typically the names registered with the detection engine
//! (e.g. `"pattern"`, `"ner"`, `"llm-ner"`).
//!
//! [`TrailStep::recognition`]: nvisy_ontology::entity::TrailStep::recognition

use std::borrow::Cow;
use std::collections::HashMap;

use async_trait::async_trait;
use nvisy_ontology::entity::{Entity, TrailStep};
use nvisy_ontology::modality::Modality;
use nvisy_ontology::primitive::Confidence;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::layer::{Layer, LayerContext};
use crate::core::ValueAt;

const TARGET: &str = "nvisy_engine::deduplication::calibrate";

/// Per-recognizer confidence multipliers applied before deduplication.
///
/// Maps a recognizer source name to a scaling factor. Recognizers
/// not present in the map are left unchanged (implicit multiplier of
/// `1.0`).
///
/// Keys use [`Cow<'static, str>`]: the canonical recognizer names are
/// `'static` string literals (`"pattern"`, `"ner"`) so they go in as
/// borrowed; custom user-supplied names from runtime config still go
/// in as owned.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct CalibrationMap(HashMap<Cow<'static, str>, f64>);

impl CalibrationMap {
    /// Empty calibration map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a multiplier for a recognizer name.
    pub fn insert(&mut self, recognizer: impl Into<Cow<'static, str>>, multiplier: f64) {
        self.0.insert(recognizer.into(), multiplier);
    }

    /// Look up the multiplier for a recognizer name, or `None`.
    pub fn get(&self, recognizer: &str) -> Option<f64> {
        self.0.get(recognizer).copied()
    }

    /// True when no multipliers are registered.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Number of registered multipliers.
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<K, V> FromIterator<(K, V)> for CalibrationMap
where
    K: Into<Cow<'static, str>>,
    V: Into<f64>,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self(
            iter.into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        )
    }
}

/// [`Layer`] that scales per-entity confidence by per-recognizer
/// calibration multipliers in place.
///
/// For each entity, finds the maximum multiplier across its
/// recognition trail step sources and scales the confidence. Entities
/// whose recognizers are absent from the map are left unchanged.
/// Results are clamped to `[0.0, 1.0]`. Adjusted entities receive a
/// [`Calibration`]
/// step on their trail.
///
/// Drops nothing — returns an empty vec from [`Layer::apply`].
///
/// [`Calibration`]: nvisy_ontology::entity::TrailStepKind::Calibration
pub struct CalibrateLayer {
    calibration: CalibrationMap,
}

impl CalibrateLayer {
    /// Construct a calibration layer from a map.
    pub fn new(calibration: CalibrationMap) -> Self {
        Self { calibration }
    }
}

#[async_trait]
impl<M: Modality, R: ValueAt<M> + ?Sized> Layer<M, R> for CalibrateLayer {
    async fn apply(
        &self,
        entities: &mut Vec<Entity<M>>,
        _ctx: &LayerContext<'_, M, R>,
    ) -> Vec<Entity<M>> {
        if self.calibration.is_empty() {
            return Vec::new();
        }

        let mut adjusted = 0usize;

        for entity in entities.iter_mut() {
            let multiplier = entity
                .recognizers()
                .filter_map(|name| self.calibration.get(name))
                .reduce(f64::max);

            if let Some(m) = multiplier {
                let before = entity.confidence;
                let after_raw = (before.get() * m).clamp(0.0, 1.0);
                let after = Confidence::new(after_raw).expect("clamped to [0,1]");
                entity.confidence = after;
                entity.trail.push(TrailStep::calibration(
                    before,
                    after,
                    format!("scaled by {m:.3}"),
                ));
                adjusted += 1;

                tracing::trace!(
                    target: TARGET,
                    entity_id = %entity.id,
                    multiplier = m,
                    before = before.get(),
                    after = after.get(),
                    "calibrated confidence",
                );
            }
        }

        tracing::debug!(
            target: TARGET,
            total = entities.len(),
            adjusted,
            "calibration pass complete",
        );

        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{
        Entity, ModelProvenance, TrailProvenance, TrailStep, TrailStepKind,
    };
    use nvisy_ontology::modality::Text;
    use nvisy_ontology::primitive::Confidence;

    use super::*;
    use crate::deduplication::test_resolver;

    fn conf(v: f64) -> Confidence {
        Confidence::new(v).expect("confidence in [0,1]")
    }

    fn trail_for(source: &str, confidence: Confidence) -> Vec<TrailStep> {
        vec![TrailStep::recognition(
            source,
            confidence,
            TrailProvenance::Model(ModelProvenance::new(source)),
            "",
        )]
    }

    async fn apply<M: Modality>(layer: &CalibrateLayer, entities: &mut Vec<Entity<M>>) {
        let resolver = test_resolver::<M>();
        let ctx = LayerContext::new(&*resolver);
        let dropped = layer.apply(entities, &ctx).await;
        assert!(dropped.is_empty(), "calibrate never drops");
    }

    #[tokio::test]
    async fn scales_confidence_and_appends_calibration_step() {
        let mut calibration = CalibrationMap::new();
        calibration.insert("pattern", 0.5);
        let layer = CalibrateLayer::new(calibration);
        let mut entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 4)
                .with_trail(trail_for("pattern", conf(0.8)))
                .with_confidence(conf(0.8))
                .test_build(),
        ];
        apply(&layer, &mut entities).await;
        assert!((entities[0].confidence.get() - 0.4).abs() < f64::EPSILON);
        assert!(
            entities[0]
                .trail
                .iter()
                .any(|s| matches!(s.kind, TrailStepKind::Calibration))
        );
    }

    #[tokio::test]
    async fn clamps_to_one() {
        let mut calibration = CalibrationMap::new();
        calibration.insert("pattern", 2.0);
        let layer = CalibrateLayer::new(calibration);
        let mut entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 4)
                .with_trail(trail_for("pattern", conf(0.8)))
                .with_confidence(conf(0.8))
                .test_build(),
        ];
        apply(&layer, &mut entities).await;
        assert!((entities[0].confidence.get() - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn picks_max_multiplier_across_recognizers() {
        let mut calibration = CalibrationMap::new();
        calibration.insert("pattern", 0.5);
        calibration.insert("ner", 0.8);
        let layer = CalibrateLayer::new(calibration);
        let mut trail = trail_for("pattern", conf(1.0));
        trail.extend(trail_for("ner", conf(1.0)));
        let mut entities: Vec<Entity<Text>> = vec![
            Entity::test_builder(0, 4)
                .with_trail(trail)
                .with_confidence(conf(1.0))
                .test_build(),
        ];
        apply(&layer, &mut entities).await;
        // max(0.5, 0.8) = 0.8; 1.0 * 0.8 = 0.8
        assert!((entities[0].confidence.get() - 0.8).abs() < f64::EPSILON);
    }
}
