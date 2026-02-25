//! Structured output types for OCR entity detection.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use nvisy_ontology::entity::{EntityCategory, EntityKind};

/// Top-level output from the OCR agent.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct OcrOutput {
    /// Full text extracted from the image.
    pub extracted_text: String,
    /// Entities detected in the extracted text.
    pub entities: Vec<RawOcrEntity>,
}

/// A single entity detected in OCR-extracted text.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct RawOcrEntity {
    /// Broad classification.
    pub category: EntityCategory,
    /// Specific entity type.
    pub entity_type: EntityKind,
    /// The matched text value.
    pub value: String,
    /// Detection confidence (0.0 -- 1.0).
    pub confidence: f64,
    /// Optional bounding box `[x, y, width, height]` in pixels.
    pub bbox: Option<[f64; 4]>,
}
