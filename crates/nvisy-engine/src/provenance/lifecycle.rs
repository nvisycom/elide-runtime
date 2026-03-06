//! Data for I/O lifecycle operations.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Data specific to I/O lifecycle operations (ingest, publish, etc.).
///
/// Duration and error are tracked on [`FileAuditEntry`](super::FileAuditEntry).
#[derive(Debug, Clone, Default)]
#[derive(Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "LifecycleActionBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleAction {
    /// Human-readable description of the operation.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Number of bytes involved.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_count: Option<u64>,
    /// Where the content came from (URI, path, bucket, etc.).
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Where the content was sent to.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    /// Content integrity checksum (e.g. SHA-256 hex digest).
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}
