//! Structured output types for NER entity detection.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use nvisy_ontology::entity::{EntityCategory, EntityKind};

/// A list of NER entities returned by structured output.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct NerEntities {
    /// Detected entities.
    pub entities: Vec<NerEntity>,
}

/// A single NER entity from structured LLM output.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct NerEntity {
    /// Broad classification.
    pub category: EntityCategory,
    /// Specific entity type.
    pub entity_type: EntityKind,
    /// The matched text value.
    pub value: String,
    /// Detection confidence (0.0..=1.0).
    pub confidence: f64,
    /// Start byte offset in the input text.
    pub start_offset: usize,
    /// End byte offset in the input text.
    pub end_offset: usize,
}
