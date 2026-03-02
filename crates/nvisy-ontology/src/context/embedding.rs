//! Embedding reference data.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Pre-computed embedding vector.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingData {
    /// The embedding vector values.
    pub vector: Vec<f64>,
    /// Dimensionality of the vector.
    pub dimensions: u32,
}
