//! Top-level audit entry.

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::FileAuditEntryKind;

/// A single processing-log entry within a [`FileAudit`](super::FileAudit).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileAuditEntry {
    /// When the operation occurred.
    #[schemars(with = "String")]
    pub timestamp: Timestamp,
    /// What kind of operation was performed, with associated data.
    #[serde(flatten)]
    pub kind: FileAuditEntryKind,
}

impl FileAuditEntry {
    /// Create a new audit entry with the given kind and current timestamp.
    pub fn new(kind: FileAuditEntryKind) -> Self {
        Self {
            timestamp: Timestamp::now(),
            kind,
        }
    }
}
