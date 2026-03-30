//! Structured output types for OCR entity verification.

use std::collections::HashMap;

use nvisy_ontology::entity::{
    Entity, EntityCategory, EntityKind, ExtractionMethod, ImageLocation, RecognitionMethod,
    RefinementMethod,
};
use nvisy_ontology::math::BoundingBox;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Whether a proposed entity was corrected or rejected by the VLM.
///
/// Entities that are confirmed are omitted from the output entirely —
/// only changed entities appear in [`VerificationOutput`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// The entity value or classification was corrected.
    Corrected,
    /// The entity was rejected as a false positive.
    Rejected,
}

/// A single entity whose status changed during VLM verification.
///
/// The `id` field is the index into the proposed entity slice, so the
/// caller can diff against the original list.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct VerifiedEntity {
    /// Index into the proposed entity list.
    pub id: usize,
    /// Whether this entity was corrected or rejected.
    pub status: VerificationStatus,
    /// Corrected category (present when `status` is `Corrected`).
    pub category: Option<EntityCategory>,
    /// Corrected entity type (present when `status` is `Corrected`).
    pub entity_type: Option<EntityKind>,
    /// Corrected value (present when `status` is `Corrected`).
    pub value: Option<String>,
    /// VLM confidence in the verdict (0.0..=1.0).
    pub confidence: f64,
    /// Corrected bounding box (present when `status` is `Corrected`).
    pub bbox: Option<BoundingBox>,
    /// Optional rationale for the correction or rejection.
    pub reason: Option<String>,
}

impl VerifiedEntity {
    /// Apply this verdict to the original entity.
    ///
    /// Returns `None` for rejected entities, or `Some(corrected)` with
    /// updated fields for corrected entities.
    pub fn apply(self, entity: Entity) -> Option<Entity> {
        match self.status {
            VerificationStatus::Rejected => None,
            VerificationStatus::Corrected => {
                let location = if let Some(bbox) = self.bbox {
                    Some(
                        ImageLocation {
                            bounding_box: bbox,
                            image_id: None,
                            page_number: None,
                        }
                        .into(),
                    )
                } else {
                    entity.location
                };

                let mut builder = Entity::builder()
                    .with_source(entity.source)
                    .with_category(self.category.unwrap_or(entity.category))
                    .with_entity_kind(self.entity_type.unwrap_or(entity.entity_kind))
                    .with_value(self.value.as_deref().unwrap_or(&entity.value).to_owned())
                    .with_recognition_methods(vec![RecognitionMethod::Ner])
                    .with_extraction_methods(vec![ExtractionMethod::OpticalCharacterRecognition])
                    .with_refinement_methods(vec![RefinementMethod::ModelVerification])
                    .with_confidence(self.confidence);

                if let Some(loc) = location {
                    builder = builder.with_location(loc);
                }

                let corrected = builder.build().expect("required fields provided");

                Some(corrected)
            }
        }
    }
}

/// Verification output containing only entities whose status changed.
///
/// Entities not present in this list are implicitly confirmed.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct VerificationOutput {
    /// Only entities that were corrected or rejected.
    pub entities: Vec<VerifiedEntity>,
}

impl VerificationOutput {
    /// Merge verdicts into the original entity list.
    ///
    /// Confirmed entities (absent from this output) pass through unchanged,
    /// corrected entities are updated, and rejected entities are dropped.
    pub fn merge(self, entities: Vec<Entity>) -> Vec<Entity> {
        let mut verdicts: HashMap<usize, VerifiedEntity> =
            self.entities.into_iter().map(|v| (v.id, v)).collect();

        let mut result = Vec::with_capacity(entities.len());
        for (i, entity) in entities.into_iter().enumerate() {
            match verdicts.remove(&i) {
                None => result.push(entity),
                Some(verified) => {
                    if let Some(corrected) = verified.apply(entity) {
                        result.push(corrected);
                    }
                }
            }
        }
        result
    }
}
