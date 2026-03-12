//! Computed representations for similarity search and pattern matching.

mod embedding;
mod pattern;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::embedding::EmbeddingData;
pub use self::pattern::PatternData;

/// Analytic computation variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnalyticVariant {
    /// Pre-computed embedding vector for similarity search.
    Embedding(EmbeddingData),
    /// Regex or glob pattern for matching.
    Pattern(PatternData),
}
