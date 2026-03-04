//! Top-level audit entry.

use std::time::Duration;

use derive_builder::Builder;
use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{DurationMicroSeconds, serde_as};
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
#[serde_as]
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "FileAuditEntryBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate"),
)]
#[serde(rename_all = "camelCase")]
pub struct FileAuditEntry {
    /// When the operation occurred.
    #[builder(default = "Timestamp::now()")]
    #[schemars(with = "String")]
    pub timestamp: Timestamp,
    /// Outcome of the operation.
    #[builder(default = "AuditEntryStatus::Success")]
    pub status: AuditEntryStatus,
    /// Wall-clock duration of the operation.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<DurationMicroSeconds>")]
    #[schemars(with = "Option<u64>")]
    pub duration: Option<Duration>,
    /// Error message if the operation failed or partially failed.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Correlation identifier for tracing across services.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Uuid>,
    /// Identifier of the policy that was evaluated, if applicable.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<Uuid>,
    /// What kind of operation was performed, with associated data.
    #[serde(flatten)]
    pub kind: FileAuditEntryKind,
}

impl FileAuditEntryBuilder {
    /// If an error is set but the status was not explicitly overridden,
    /// default to [`AuditEntryStatus::Failed`].
    fn validate(&self) -> Result<(), String> {
        if self.error.is_some() && self.status.is_none() {
            // Cannot mutate in validate; we handle this in a custom build wrapper.
        }
        Ok(())
    }

    /// Build the entry, automatically marking it as failed when an error is set
    /// and no explicit status was provided.
    pub fn finish(mut self) -> Result<FileAuditEntry, FileAuditEntryBuilderError> {
        if self.error.is_some() && self.status.is_none() {
            self.status = Some(AuditEntryStatus::Failed);
        }
        self.build()
    }
}
