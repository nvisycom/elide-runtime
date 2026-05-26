//! Per-method confidence calibration.
//!
//! Scales entity confidence scores using per-[`RecognitionMethod`]
//! multipliers before deduplication. This compensates for score
//! distribution differences between detectors: regex always returns
//! 1.0 while NER returns 0.3–0.9, so a multiplier of 0.8 on regex
//! brings them into alignment.
//!
//! [`RecognitionMethod`]: nvisy_ontology::entity::RecognitionMethod

use std::collections::HashMap;

use nvisy_ontology::entity::{RecognitionMethodKind, RefinementMethod};
use nvisy_ontology::modality::AnyModality;
use nvisy_ontology::primitive::Confidence;

const TARGET: &str = "nvisy_engine::op::deduplication::calibration";

/// Per-method confidence multiplier applied before deduplication.
///
/// Maps a [`RecognitionMethodKind`] to a scaling factor. Methods not
/// present in the map are left unchanged (implicit multiplier of 1.0).
pub type CalibrationMap = HashMap<RecognitionMethodKind, f64>;

/// Extension trait on [`Entities`]: scale per-entity confidence by
/// per-method calibration multipliers in place.
pub(crate) trait Calibrate {
    /// Apply per-method calibration multipliers to entity confidences.
    ///
    /// For each entity, finds the maximum multiplier across its
    /// recognition methods and scales the confidence. Entities whose
    /// methods are absent from the map are left unchanged. Results
    /// are clamped to `[0.0, 1.0]`. Adjusted entities are tagged with
    /// [`RefinementMethod::ConfidenceCalibration`].
    fn calibrate(&mut self, calibration: &CalibrationMap);
}

impl Calibrate for Vec<Entity<AnyModality>> {
    fn calibrate(&mut self, calibration: &CalibrationMap) {
        if calibration.is_empty() {
            return;
        }

        let mut adjusted = 0usize;

        for entity in self.iter_mut() {
            let multiplier = entity
                .recognition_methods
                .iter()
                .filter_map(|m| calibration.get(&m.kind()).copied())
                .reduce(f64::max);

            if let Some(m) = multiplier {
                let before = entity.confidence.get();
                let after = (before * m).clamp(0.0, 1.0);
                entity.confidence = Confidence::new(after).expect("clamped to [0,1]");
                entity
                    .refinement_methods
                    .push(RefinementMethod::ConfidenceCalibration);
                adjusted += 1;

                tracing::trace!(
                    target: TARGET,
                    entity_id = %entity.id,
                    multiplier = m,
                    before,
                    after,
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
    use nvisy_ontology::entity::{Entity, ModelKind, RecognitionMethod, RecognitionMethodKind};
    use nvisy_ontology::primitive::Confidence;

    use super::*;

    fn conf(v: f64) -> Confidence {
        Confidence::new(v).expect("confidence in [0,1]")
    }

    #[test]
    fn scales_confidence_and_tags_refinement() {
        let mut calibration = CalibrationMap::new();
        calibration.insert(RecognitionMethodKind::Pattern, 0.5);
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.8))
                .test_build(),
        ]
        .into();
        entities.calibrate(&calibration);
        assert!((entities[0].confidence.get() - 0.4).abs() < f64::EPSILON);
        assert!(
            entities[0]
                .refinement_methods
                .contains(&RefinementMethod::ConfidenceCalibration)
        );
    }

    #[test]
    fn clamps_to_one() {
        let mut calibration = CalibrationMap::new();
        calibration.insert(RecognitionMethodKind::Pattern, 2.0);
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_confidence(conf(0.8))
                .test_build(),
        ]
        .into();
        entities.calibrate(&calibration);
        assert!((entities[0].confidence.get() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn picks_max_multiplier_across_methods() {
        let mut calibration = CalibrationMap::new();
        calibration.insert(RecognitionMethodKind::Pattern, 0.5);
        calibration.insert(RecognitionMethodKind::NlpNer, 0.8);
        let mut entities: Vec<_> = vec![
            Entity::test_builder(0, 4)
                .with_recognition_methods(vec![
                    RecognitionMethod::regex("test"),
                    RecognitionMethod::nlp_ner("test", ModelKind::SelfHosted),
                ])
                .with_confidence(conf(1.0))
                .test_build(),
        ]
        .into();
        entities.calibrate(&calibration);
        // max(0.5, 0.8) = 0.8; 1.0 * 0.8 = 0.8
        assert!((entities[0].confidence.get() - 0.8).abs() < f64::EPSILON);
    }
}
