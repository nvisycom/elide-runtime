//! Structured output types for NER entity detection.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use nvisy_ontology::entity::{EntityCategory, EntityKind};

/// A list of raw entities returned by structured output.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct RawEntities {
    /// Detected entities.
    pub entities: Vec<RawEntity>,
}

/// A single raw entity from structured LLM output.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct RawEntity {
    /// Broad classification.
    pub category: EntityCategory,
    /// Specific entity type.
    pub entity_type: EntityKind,
    /// The matched text value.
    pub value: String,
    /// Detection confidence (0.0 -- 1.0).
    pub confidence: f64,
    /// Start byte offset in the input text.
    pub start_offset: usize,
    /// End byte offset in the input text.
    pub end_offset: usize,
}
