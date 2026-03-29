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
    pub vector: Vec<f64>,
    /// Dimensionality of the vector.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    /// Identifier of the model that produced this embedding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Distance metric to use for comparison.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_metric: Option<DistanceMetric>,
}
