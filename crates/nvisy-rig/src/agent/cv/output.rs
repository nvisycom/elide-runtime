//! Structured output types for CV detection.

use nvisy_ontology::entity::{EntityCategory, EntityKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A single entity detected by computer vision.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct CvEntity {
    /// Broad classification.
    pub category: EntityCategory,
    /// Specific entity type.
    pub entity_type: EntityKind,
    /// Label from the CV model (e.g. "face", "license_plate").
    pub label: String,
    /// Detection confidence (0.0..=1.0).
    pub confidence: f64,
    /// Bounding box `[x, y, width, height]` in pixels.
    pub bbox: [f64; 4],
}

/// Wrapper for structured output parsing.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct CvEntities {
    /// Detected entities.
    pub entities: Vec<CvEntity>,
}
