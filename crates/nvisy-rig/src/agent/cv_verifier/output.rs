//! CV-specific verdict application + output re-exports.
//!
//! The verdict shape (`VerificationStatus`, `VerifiedEntity`,
//! `VerificationOutput`) is shared with [`NerVerifier`] and lives
//! in [`crate::agent::base::verification`]. CV-specific *apply*
//! logic — translating a bbox into an [`ImageLocation`] update on
//! the original entity — lives here.
//!
//! [`NerVerifier`]: crate::agent::NerVerifier
//! [`ImageLocation`]: nvisy_ontology::entity::ImageLocation

use std::collections::HashMap;

use nvisy_ontology::entity::{Entity, ImageLocation, RefinementMethod};
use nvisy_ontology::primitive::Confidence;

pub use crate::agent::base::{VerificationOutput, VerificationStatus, VerifiedEntity};

/// Merge CV verifier verdicts into the original entity list.
///
/// Confirmed entities (absent from `output`) pass through
/// unchanged; corrected entities are updated with the verifier's
/// bbox / value / type / category; rejected entities are dropped.
pub(super) fn merge(output: VerificationOutput, entities: Vec<Entity>) -> Vec<Entity> {
    let mut verdicts: HashMap<usize, VerifiedEntity> =
        output.entities.into_iter().map(|v| (v.id, v)).collect();

    let mut result = Vec::with_capacity(entities.len());
    for (i, entity) in entities.into_iter().enumerate() {
        match verdicts.remove(&i) {
            None => result.push(entity),
            Some(verified) => {
                if let Some(corrected) = apply(verified, entity) {
                    result.push(corrected);
                }
            }
        }
    }
    result
}

/// Apply one CV verifier verdict to one original entity.
///
/// Returns `None` for rejected entities, or `Some(corrected)` with
/// updated fields for corrected entities. Bounding-box updates
/// rebuild the entity's [`ImageLocation`].
fn apply(verified: VerifiedEntity, entity: Entity) -> Option<Entity> {
    match verified.status {
        VerificationStatus::Rejected => None,
        VerificationStatus::Corrected => {
            let location = if let Some(bbox) = verified.bbox {
                ImageLocation {
                    bounding_box: bbox,
                    image_id: None,
                    page_number: None,
                }
                .into()
            } else {
                entity.location
            };

            let mut refinements = entity.refinement_methods;
            refinements.push(RefinementMethod::ModelVerification);
            // Model-reported scores aren't guaranteed in range;
            // clamp defensively before constructing.
            let confidence = Confidence::new(verified.confidence.clamp(0.0, 1.0))
                .expect("clamped value is in [0,1]");
            let mut b = Entity::builder()
                .with_id(entity.id)
                .with_category(verified.category.unwrap_or(entity.category))
                .with_entity_kind(verified.entity_type.unwrap_or(entity.entity_kind))
                .with_recognition_methods(entity.recognition_methods)
                .with_extraction_methods(entity.extraction_methods)
                .with_refinement_methods(refinements)
                .with_confidence(confidence)
                .with_location(location);
            if let Some(id) = entity.entity_id {
                b = b.with_entity_id(id);
            }
            Some(b.build().expect("required fields provided"))
        }
    }
}
