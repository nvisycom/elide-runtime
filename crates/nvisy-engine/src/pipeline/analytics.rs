//! Aggregate pipeline analytics.
//!
//! [`AnalyticsSnapshot`] captures point-in-time metrics across all tracked
//! pipeline runs — counts by status, entity/redaction totals, distinct
//! actors, and min/max/avg run durations. The snapshot is computed on
//! demand from the in-memory [`RunState`](super::runs::state::RunState)
//! and is exposed via [`Engine::snapshot`](super::Engine::snapshot).

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Point-in-time aggregate metrics across all pipeline runs.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSnapshot {
    /// Timestamp when the snapshot was taken.
    #[schemars(with = "String")]
    pub timestamp: Timestamp,
    /// Total number of pipeline runs tracked by the engine.
    pub total_runs: u64,
    /// Number of runs currently in progress.
    pub active_runs: u64,
    /// Number of runs that completed successfully.
    pub succeeded_runs: u64,
    /// Number of runs that failed (fully or partially).
    pub failed_runs: u64,
    /// Number of runs that were cancelled.
    pub cancelled_runs: u64,
    /// Total number of entities detected across all completed runs.
    pub total_entities_detected: u64,
    /// Total number of redactions applied across all completed runs.
    pub total_redactions_applied: u64,
    /// Number of distinct actors that have triggered runs.
    pub distinct_actors: u64,
    /// Shortest completed run duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_run_duration_ms: Option<u64>,
    /// Longest completed run duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_run_duration_ms: Option<u64>,
    /// Average completed run duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_run_duration_ms: Option<f64>,
}
