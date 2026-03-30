//! Top-level audit entry.

use std::time::Duration;

use derive_builder::Builder;
use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{DurationMicroSeconds, serde_as};
use strum::{Display, EnumString};
use uuid::Uuid;

use super::AuditEntryKind;

/// Outcome status of an audit entry operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum AuditEntryStatus {
    /// Operation completed successfully.
    Success,
    /// Operation failed.
    Failed,
    /// Operation completed with partial results.
    Partial,
}

/// A single processing-log entry within an [`Audit`].
///
/// [`Audit`]: super::Audit
#[serde_as]
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "AuditEntryBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(private, name = "build_inner")
)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
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
    pub kind: AuditEntryKind,
}

impl AuditEntryBuilder {
    /// Build the entry.
    ///
    /// When an error is set and no explicit status was provided, the status
    /// is automatically set to [`AuditEntryStatus::Failed`].
    pub fn build(mut self) -> Result<AuditEntry, AuditEntryBuilderError> {
        if self.error.is_some() && self.status.is_none() {
            self.status = Some(AuditEntryStatus::Failed);
        }
        self.build_inner()
    }
}
