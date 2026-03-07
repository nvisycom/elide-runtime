//! Input types for OCR verification.

use nvisy_core::math::BoundingBox;
use nvisy_ontology::entity::{Entity, EntityCategory, EntityKind, Location};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An entity proposed by NER that the VLM should verify against the image.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct ProposedEntity {
    /// Index used to correlate with [`VerifiedEntity::id`](super::VerifiedEntity::id).
    pub id: usize,
    /// Broad classification.
    pub category: EntityCategory,
    /// Specific entity type.
    pub entity_type: EntityKind,
    /// The matched text value.
    pub value: String,
    /// Detection confidence (0.0..=1.0).
    pub confidence: f64,
    /// Axis-aligned bounding box in pixels.
    pub bbox: Option<BoundingBox>,
}

impl ProposedEntity {
    /// Create a proposed entity from a detected [`Entity`] and its index.
    pub fn from_entity(id: usize, entity: &Entity) -> Self {
        let bbox = match &entity.location {
            Some(Location::Image(loc)) => Some(loc.bounding_box),
            _ => None,
        };
        Self {
            id,
            category: entity.category.clone(),
            entity_type: entity.entity_kind,
            value: entity.value.clone(),
            confidence: entity.confidence,
            bbox,
        }
    }
}
