//! Input types for OCR verification.

use nvisy_ontology::entity::{Entity, EntityCategory, EntityKind, Location};
use nvisy_ontology::primitive::BoundingBox;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An entity paired with its resolved text value, ready for VLM
/// verification.
pub struct VerificationCandidate {
    /// The detected entity.
    pub entity: Entity,
    /// Text value resolved from the document.
    pub value: String,
}

/// An entity proposed by NER that the VLM should verify against the image.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct ProposedEntity {
    /// Index used to correlate with [`VerifiedEntity::id`].
    ///
    /// [`VerifiedEntity::id`]: crate::agent::VerifiedEntity::id
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
    /// Create a proposed entity from a detected [`Entity`], its index,
    /// and the resolved text value from the document.
    pub fn from_entity(id: usize, entity: &Entity, value: &str) -> Self {
        let bbox = match &entity.location {
            Location::Image(loc) => Some(loc.bounding_box),
            _ => None,
        };
        Self {
            id,
            category: entity.category,
            entity_type: entity.entity_kind,
            value: value.to_string(),
            confidence: entity.confidence.get(),
            bbox,
        }
    }
}
