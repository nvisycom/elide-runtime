//! CV-specific verdict application + output re-exports.
//!
//! The verdict shape (`VerificationStatus`, `VerifiedEntity`,
//! `VerificationOutput`) is shared with [`NerVerifyAgent`] and lives
//! in [`base::verification`]. CV-specific *apply* logic — translating
//! a bbox into an [`Image`] update on the original entity —
//! lives here.
//!
//! [`NerVerifyAgent`]: crate::agent::ner::NerVerifyAgent
//! [`base::verification`]: crate::agent::base::verification
//! [`Image`]: nvisy_ontology::modality::Image

use std::collections::HashMap;

use nvisy_ontology::entity::{Entity, ModelProvenance, TrailProvenance, TrailStep};
use nvisy_ontology::modality::Image;
use nvisy_ontology::primitive::Confidence;

pub use crate::agent::base::{VerificationOutput, VerificationStatus, VerifiedEntity};

/// Merge CV verifier verdicts into the original entity list.
///
/// Confirmed entities (absent from `output`) pass through
/// unchanged; corrected entities are updated with the verifier's
/// bbox / value / type; rejected entities are dropped.
pub(super) fn merge(
    output: VerificationOutput,
    entities: Vec<Entity<Image>>,
    verifier: &ModelProvenance,
) -> Vec<Entity<Image>> {
    let mut verdicts: HashMap<usize, VerifiedEntity> =
        output.entities.into_iter().map(|v| (v.id, v)).collect();

    let mut result = Vec::with_capacity(entities.len());
    for (i, entity) in entities.into_iter().enumerate() {
        match verdicts.remove(&i) {
            None => result.push(entity),
            Some(verified) => {
                if let Some(corrected) = apply(verified, entity, verifier) {
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
/// rebuild the entity's [`Image`] location, and a
/// [`Verification`]
/// step is appended to the entity's trail with the verifier's
/// provenance.
///
/// [`Verification`]: nvisy_ontology::entity::TrailStepKind::Verification
fn apply(
    verified: VerifiedEntity,
    mut entity: Entity<Image>,
    verifier: &ModelProvenance,
) -> Option<Entity<Image>> {
    match verified.status {
        VerificationStatus::Rejected => None,
        VerificationStatus::Corrected => {
            let original = entity.confidence;
            let adjusted =
                Confidence::new(verified.confidence.get().clamp(0.0, 1.0)).unwrap_or(original);

            if let Some(bbox) = verified.bbox {
                entity.location = Image {
                    bounding_box: bbox,
                    polygon: None,
                    image_id: None,
                    page_number: None,
                };
            }
            if let Some(kind) = verified.entity_type {
                entity.entity_kind = kind;
            }
            entity.confidence = adjusted;
            let reason = verified
                .reason
                .unwrap_or_else(|| "verifier corrected".to_owned());
            entity.trail.push(TrailStep::verification(
                "vlm-verify",
                original,
                adjusted,
                TrailProvenance::Model(verifier.clone()),
                reason,
            ));
            Some(entity)
        }
    }
}
