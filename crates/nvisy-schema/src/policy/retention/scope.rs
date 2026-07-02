//! [`RetentionScope`]: what class of data a retention rule
//! applies to.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// What class of data a retention policy applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display)]
#[derive(EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum RetentionScope {
    /// Original ingested content before redaction.
    OriginalContent,
    /// Redacted output artifacts.
    RedactedOutput,
    /// Audit log entries.
    AuditLogs,
}
