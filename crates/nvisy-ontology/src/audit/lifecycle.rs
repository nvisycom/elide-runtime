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
    /// Where the content came from (URI, path, bucket, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Where the content was sent to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    /// Content integrity checksum (e.g. SHA-256 hex digest).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

impl LifecycleAction {
    /// Create a new lifecycle action.
    pub fn new() -> Self {
        Self {
            description: None,
            byte_count: None,
            source: None,
            destination: None,
            checksum: None,
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

    /// Set the source location.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set the destination location.
    pub fn with_destination(mut self, destination: impl Into<String>) -> Self {
        self.destination = Some(destination.into());
        self
    }

    /// Set the content integrity checksum.
    pub fn with_checksum(mut self, checksum: impl Into<String>) -> Self {
        self.checksum = Some(checksum.into());
        self
    }
}

impl Default for LifecycleAction {
    fn default() -> Self {
        Self::new()
    }
}
