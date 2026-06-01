//! Per-recognizer confidence calibration.
//!
//! Scales entity confidence scores using per-recognizer multipliers
//! before deduplication. This compensates for score distribution
//! differences between detectors: regex always returns 1.0 while NER
//! returns 0.3–0.9, so a multiplier of 0.8 on `pattern` brings them
//! into alignment.
//!
//! Keys are the recognizer source names stamped onto the entity's
//! [`TrailStep::recognition`](nvisy_ontology::entity::TrailStep::recognition)
//! step — typically the names registered with the detection engine
//! (e.g. `"pattern"`, `"ner"`, `"llm-ner"`).

use std::collections::HashMap;

use nvisy_ontology::entity::{Entity, TrailStep};
use nvisy_ontology::modality::Modality;
use nvisy_ontology::primitive::Confidence;

const TARGET: &str = "nvisy_engine::op::deduplication::calibration";

/// Per-recognizer confidence multiplier applied before deduplication.
///
/// Maps a recognizer source name to a scaling factor. Recognizers
/// not present in the map are left unchanged (implicit multiplier of
/// `1.0`).
pub type CalibrationMap = HashMap<String, f64>;

/// Extension trait on entity collections: scale per-entity confidence
/// by per-recognizer calibration multipliers in place.
pub(super) trait Calibrate {
    /// Apply per-recognizer calibration multipliers to entity
    /// confidences.
    ///
    /// For each entity, finds the maximum multiplier across its
    /// recognition trail step sources and scales the confidence.
    /// Entities whose recognizers are absent from the map are left
    /// unchanged. Results are clamped to `[0.0, 1.0]`. Adjusted
    /// entities receive a
    /// [`Calibration`](nvisy_ontology::entity::TrailStepKind::Calibration)
    /// step on their trail.
    fn calibrate(&mut self, calibration: &CalibrationMap);
}

impl<M: Modality> Calibrate for Vec<Entity<M>> {
    fn calibrate(&mut self, calibration: &CalibrationMap) {
        if calibration.is_empty() {
            return;
        }

        let mut adjusted = 0usize;

        for entity in self.iter_mut() {
            let multiplier = entity
                .recognizers()
                .filter_map(|name| calibration.get(name).copied())
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
            total = self.len(),
            adjusted,
            "calibration pass complete",
        );
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{
        Entity, ModelProvenance, TrailProvenance, TrailStep, TrailStepKind,
    };
    use nvisy_ontology::primitive::Confidence;

    use super::*;

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

    #[test]
    fn scales_confidence_and_appends_calibration_step() {
        let mut calibration = CalibrationMap::new();
        calibration.insert("pattern".into(), 0.5);
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_trail(trail_for("pattern", conf(0.8)))
                .with_confidence(conf(0.8))
                .test_build(),
        ];
        entities.calibrate(&calibration);
        assert!((entities[0].confidence.get() - 0.4).abs() < f64::EPSILON);
        assert!(
            entities[0]
                .trail
                .iter()
                .any(|s| matches!(s.kind, TrailStepKind::Calibration))
        );
    }

    #[test]
    fn clamps_to_one() {
        let mut calibration = CalibrationMap::new();
        calibration.insert("pattern".into(), 2.0);
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_trail(trail_for("pattern", conf(0.8)))
                .with_confidence(conf(0.8))
                .test_build(),
        ];
        entities.calibrate(&calibration);
        assert!((entities[0].confidence.get() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn picks_max_multiplier_across_recognizers() {
        let mut calibration = CalibrationMap::new();
        calibration.insert("pattern".into(), 0.5);
        calibration.insert("ner".into(), 0.8);
        let mut trail = trail_for("pattern", conf(1.0));
        trail.extend(trail_for("ner", conf(1.0)));
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_trail(trail)
                .with_confidence(conf(1.0))
                .test_build(),
        ];
        entities.calibrate(&calibration);
        // max(0.5, 0.8) = 0.8; 1.0 * 0.8 = 0.8
        assert!((entities[0].confidence.get() - 0.8).abs() < f64::EPSILON);
    }
}
