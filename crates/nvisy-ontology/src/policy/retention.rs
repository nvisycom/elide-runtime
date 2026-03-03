//! Data retention policy types.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// What class of data a retention policy applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum RetentionScope {
    /// Original ingested content before redaction.
    OriginalContent,
    /// Redacted output artifacts.
    RedactedOutput,
    /// Audit log entries.
    AuditLogs,
}

/// A retention policy governing how long data is kept.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RetentionPolicy {
    /// What class of data this policy applies to.
    pub scope: RetentionScope,
    /// Maximum number of days to retain data. `None` means indefinite.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_duration_days: Option<u64>,
    /// If true, delete data immediately after processing (zero-retention mode).
    pub zero_retention: bool,
    /// Description of the retention policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl RetentionPolicy {
    /// Returns the retention duration, or `None` for indefinite retention.
    ///
    /// Returns [`Duration::ZERO`] when `zero_retention` is `true`.
    pub fn duration(&self) -> Option<Duration> {
        if self.zero_retention {
            return Some(Duration::ZERO);
        }
        self.max_duration_days
            .map(|days| Duration::from_secs(days * 24 * 60 * 60))
    }
}
