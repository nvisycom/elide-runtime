//! Computed representations for similarity search and pattern matching.

mod embedding;
mod pattern;

pub use embedding::EmbeddingData;
pub use pattern::PatternData;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Analytic computation variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnalyticVariant {
    /// Pre-computed embedding vector for similarity search.
    Embedding(EmbeddingData),
    /// Regex or glob pattern for matching.
    Pattern(PatternData),
}
