//! Data for deterministic processing operations.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Data specific to deterministic processing operations
/// (pattern matching, redaction, etc.).
///
/// Duration and error are tracked on [`FileAuditEntry`](super::FileAuditEntry).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingAction {
    /// Number of items processed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items_count: Option<u64>,
    /// Number of items that matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_count: Option<u64>,
}

impl ProcessingAction {
    /// Create a new processing action.
    pub fn new() -> Self {
        Self {
            items_count: None,
            matched_count: None,
        }
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

impl Default for ProcessingAction {
    fn default() -> Self {
        Self::new()
    }
}
