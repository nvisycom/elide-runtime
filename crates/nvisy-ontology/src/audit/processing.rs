//! Data for deterministic processing operations.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Data for deterministic processing operations (pattern matching, redaction, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingAction {
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Number of items processed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items_count: Option<u64>,
    /// Number of items that matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_count: Option<u64>,
}

impl ProcessingAction {
    /// Create a new processing action with the given duration.
    pub fn new(duration: Duration) -> Self {
        Self {
            duration_ms: duration.as_millis() as u64,
            items_count: None,
            matched_count: None,
        }
    }

    /// Wall-clock duration.
    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.duration_ms)
    }

    /// Set the number of items processed.
    pub fn with_items_count(mut self, count: u64) -> Self {
        self.items_count = Some(count);
        self
    }

    /// Set the number of items that matched.
    pub fn with_matched_count(mut self, count: u64) -> Self {
        self.matched_count = Some(count);
        self
    }
}
