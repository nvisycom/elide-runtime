//! Data retention policy types.

use std::time::Duration;

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

/// How long data is retained.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Retention {
    /// Delete data immediately after processing.
    ZeroRetention,
    /// Retain data for a fixed number of days.
    Duration {
        /// Maximum number of days to retain data.
        days: u64,
    },
    /// Retain data indefinitely.
    Indefinite,
}

impl Retention {
    /// Returns the retention duration.
    ///
    /// Returns [`Duration::ZERO`] for `ZeroRetention` and `None` for `Indefinite`.
    pub fn duration(&self) -> Option<Duration> {
        match self {
            Self::ZeroRetention => Some(Duration::ZERO),
            Self::Duration { days } => Some(Duration::from_secs(days * 24 * 60 * 60)),
            Self::Indefinite => None,
        }
    }
}

/// A retention policy governing how long a class of data is kept.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RetentionPolicy {
    /// What class of data this policy applies to.
    pub scope: RetentionScope,
    /// How long to retain data.
    pub retention: Retention,
    /// Description of the retention policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
