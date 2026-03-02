//! Data for I/O lifecycle operations.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Data for I/O lifecycle operations (ingest, publish).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleAction {
    /// Human-readable description of the operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Number of bytes involved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_count: Option<u64>,
}

impl LifecycleAction {
    /// Create a new lifecycle action.
    pub fn new() -> Self {
        Self {
            description: None,
            byte_count: None,
        }
    }

    /// Set a human-readable description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the byte count.
    pub fn with_byte_count(mut self, count: u64) -> Self {
        self.byte_count = Some(count);
        self
    }
}

impl Default for LifecycleAction {
    fn default() -> Self {
        Self::new()
    }
}
