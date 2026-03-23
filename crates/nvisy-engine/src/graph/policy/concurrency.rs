//! Concurrency policy for graph execution.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Controls how many graph nodes may execute concurrently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ConcurrencyPolicy {
    /// Maximum number of nodes executing in parallel.
    pub max_nodes: usize,
}
