//! Concurrency policy for graph execution.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Controls how many graph nodes may execute concurrently.
///
/// `max_nodes` must be at least 1; a value of 0 would deadlock the
/// pipeline since no node could ever acquire a semaphore permit.
#[derive(Debug, Clone, Copy, Validate, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ConcurrencyPolicy {
    /// Maximum number of nodes executing in parallel (must be >= 1).
    #[validate(range(min = 1))]
    pub max_nodes: usize,
}
