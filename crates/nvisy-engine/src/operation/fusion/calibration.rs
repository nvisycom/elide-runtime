//! Per-method confidence calibration.

use nvisy_ontology::entity::Entities;
use nvisy_ontology::workflow::CalibrationMap;

/// Apply per-method calibration multipliers to entity confidences.
///
/// For each entity, finds the maximum multiplier across its recognition
/// methods and scales the confidence accordingly. Entities whose methods
/// are not in the map are left unchanged. Results are clamped to `[0.0, 1.0]`.
pub(super) fn calibrate(entities: &mut Entities, calibration: &CalibrationMap) {
    for entity in entities.iter_mut() {
        let multiplier = entity
            .recognition_methods
            .iter()
            .filter_map(|m| calibration.get(m).copied())
            .reduce(f64::max);
        if let Some(m) = multiplier {
            entity.confidence = (entity.confidence * m).clamp(0.0, 1.0);
        }
    }
}
