//! Input types for OCR verification.

use nvisy_core::math::BoundingBox;
use nvisy_ontology::entity::{EntityCategory, EntityKind};
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
