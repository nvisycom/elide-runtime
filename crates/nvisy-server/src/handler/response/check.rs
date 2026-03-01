//! Check response types (health, analytics).

use schemars::JsonSchema;
use serde::Serialize;

/// Response body for `GET /health`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Health {
    /// Server status string (always `"ok"` when reachable).
    pub status: &'static str,
}

/// Response body for `GET /api/v1/analytics`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Analytics {
    /// Total number of pipeline runs.
    pub total_runs: u64,
    /// Total number of entities detected across all runs.
    pub total_entities_detected: u64,
    /// Total number of redactions applied across all runs.
    pub total_redactions_applied: u64,
}
