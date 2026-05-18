//! Embedding reference data for similarity search.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Distance metric for vector similarity comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DistanceMetric {
    /// Cosine similarity (1 − cos θ).
    Cosine,
    /// Euclidean (L2) distance.
    Euclidean,
    /// Dot product (inner product).
    DotProduct,
    /// Manhattan (L1) distance.
    Manhattan,
}

/// Pre-computed embedding vector for similarity comparison.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingData {
    /// The embedding vector values.
    pub vector: Vec<f32>,
    /// Identifier of the model/algorithm that produced this embedding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,
    /// Distance metric to use for comparison.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_metric: Option<DistanceMetric>,
}

impl EmbeddingData {
    /// Dimensionality of the embedding vector.
    pub fn dimensions(&self) -> usize {
        self.vector.len()
    }
}
