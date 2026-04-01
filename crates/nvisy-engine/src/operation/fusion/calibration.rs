//! Per-method confidence calibration.
//!
//! Scales entity confidence scores using per-[`RecognitionMethod`]
//! multipliers before fusion. This compensates for score distribution
//! differences between detectors: regex always returns 1.0 while NER
//! returns 0.3:0.9, so a multiplier of 0.8 on regex brings them into
//! alignment.
//!
//! [`RecognitionMethod`]: nvisy_ontology::entity::RecognitionMethod

use nvisy_ontology::entity::Entities;
use nvisy_ontology::workflow::CalibrationMap;

const TARGET: &str = "nvisy_engine::op::fusion::calibration";

/// Apply per-method calibration multipliers to entity confidences.
///
/// For each entity, finds the maximum multiplier across its recognition
/// methods and scales the confidence. Entities whose methods are absent
/// from the map are left unchanged. Results are clamped to `[0.0, 1.0]`.
pub(super) fn calibrate(entities: &mut Entities, calibration: &CalibrationMap) {
    let mut adjusted = 0usize;

    for entity in entities.iter_mut() {
        let multiplier = entity
            .recognition_methods
            .iter()
            .filter_map(|m| calibration.get(&m.kind()).copied())
            .reduce(f64::max);

        if let Some(m) = multiplier {
            let before = entity.confidence;
            entity.confidence = (entity.confidence * m).clamp(0.0, 1.0);
            adjusted += 1;

            tracing::trace!(
                target: TARGET,
                entity_id = %entity.id,
                multiplier = m,
                before,
                after = entity.confidence,
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
}
