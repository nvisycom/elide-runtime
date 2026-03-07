//! Check response types (health, analytics).

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Represents the operational status of a service.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    /// Service is operating normally.
    #[default]
    Healthy,
    /// Service is operating with some issues but still functional.
    Degraded,
    /// Service is not operational.
    Unhealthy,
}

/// Response body for `GET /health`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    /// Current service status.
    pub status: ServiceStatus,
    /// RFC 3339 timestamp of when the check was performed.
    #[schemars(with = "String")]
    pub timestamp: Timestamp,
}

/// Response body for `GET /api/v1/analytics`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Analytics {
    /// RFC 3339 timestamp of when the analytics were collected.
    #[schemars(with = "String")]
    pub timestamp: Timestamp,
    /// Total number of pipeline runs.
    pub total_runs: u64,
    /// Total number of entities detected across all runs.
    pub total_entities_detected: u64,
    /// Total number of redactions applied across all runs.
    pub total_redactions_applied: u64,
}
