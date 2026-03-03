//! Embedding reference data for similarity search.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Pre-computed embedding vector for similarity comparison.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingData {
    /// The embedding vector values.
    pub vector: Vec<f64>,
    /// Dimensionality of the vector.
    pub dimensions: u32,
    /// Identifier of the model that produced this embedding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Distance metric to use for comparison (e.g. `"cosine"`, `"euclidean"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_metric: Option<String>,
}
