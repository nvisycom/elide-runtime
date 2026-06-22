//! Data retention policy types.

mod duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

pub use self::duration::Retention;

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

/// A single retention rule: scope + duration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct RetentionPolicy {
    /// What class of data this applies to.
    pub scope: RetentionScope,
    /// How long to retain data.
    pub retention: Retention,
}
