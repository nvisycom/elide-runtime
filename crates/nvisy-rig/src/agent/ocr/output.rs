//! Structured output types for OCR entity detection.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use nvisy_core::math::BoundingBox;
use nvisy_ontology::entity::{EntityCategory, EntityKind};

/// Top-level output from the OCR agent.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct OcrOutput {
    /// Full text extracted from the image.
    pub extracted_text: String,
    /// Entities detected in the extracted text.
    pub entities: Vec<OcrEntity>,
}

/// A single entity detected in OCR-extracted text.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct OcrEntity {
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
