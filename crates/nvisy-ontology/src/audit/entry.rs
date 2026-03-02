//! Top-level audit entry.

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

use super::FileAuditEntryKind;

/// Outcome status of an audit entry operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AuditEntryStatus {
    /// Operation completed successfully.
    Success,
    /// Operation failed.
    Failed,
    /// Operation completed with partial results.
    Partial,
}

/// A single processing-log entry within a [`FileAudit`](super::FileAudit).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileAuditEntry {
    /// When the operation occurred.
    #[schemars(with = "String")]
    pub timestamp: Timestamp,
    /// Outcome of the operation.
    pub status: AuditEntryStatus,
    /// Correlation identifier for tracing across services.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Uuid>,
    /// What kind of operation was performed, with associated data.
    #[serde(flatten)]
    pub kind: FileAuditEntryKind,
}

impl FileAuditEntry {
    /// Create a new successful audit entry with the given kind and current timestamp.
    pub fn new(kind: FileAuditEntryKind) -> Self {
        Self {
            timestamp: Timestamp::now(),
            status: AuditEntryStatus::Success,
            correlation_id: None,
            kind,
        }
    }

    /// Set the outcome status.
    pub fn with_status(mut self, status: AuditEntryStatus) -> Self {
        self.status = status;
        self
    }

    /// Set a correlation identifier for cross-service tracing.
    pub fn with_correlation_id(mut self, id: Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }
}
