//! Concurrency policy for graph execution.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Controls how many graph nodes may execute concurrently.
///
/// `max_nodes` must be at least 1; a value of 0 would deadlock the
/// pipeline since no node could ever acquire a semaphore permit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ConcurrencyPolicy {
    /// Maximum number of nodes executing in parallel (must be >= 1).
    pub max_nodes: usize,
}

impl ConcurrencyPolicy {
    /// Validate that the policy is usable.
    ///
    /// Returns `Err` if `max_nodes` is 0.
    pub fn validate(&self) -> Result<(), nvisy_core::Error> {
        if self.max_nodes == 0 {
            return Err(nvisy_core::Error::validation(
                "concurrency max_nodes must be at least 1",
                "graph",
            ));
        }
        Ok(())
    }
}
