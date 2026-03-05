//! Structured output types for text generation.

use nvisy_ontology::entity::EntityKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A single generated entity — the original value replaced with a synthetic one.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct GeneratedEntity {
    /// The entity type that was generated.
    pub entity_type: EntityKind,
    /// The original (real) value.
    pub original_value: String,
    /// The generated synthetic replacement value.
    pub synthetic_value: String,
}

/// Wrapper for structured output containing a batch of generated entities.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
pub struct GenOutput {
    /// Generated entities.
    pub entities: Vec<GeneratedEntity>,
}
