//! Point-in-time aggregate metrics across all tracked pipeline runs.
//!
//! [`AnalyticsSnapshot`] captures status counts, actor count, and
//! duration stats. The snapshot is computed on demand from the
//! in-memory [`RunState`] and exposed
//! via [`Engine::snapshot`].
//!
//! [`RunState`]: super::runs::state::RunState
//! [`Engine::snapshot`]: super::Engine::snapshot

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
    /// Number of runs currently in progress.
    pub current_runs: u64,
    /// Number of runs that completed successfully.
    pub succeeded_runs: u64,
    /// Number of runs that failed (fully or partially).
    pub failed_runs: u64,
    /// Number of runs that were cancelled.
    pub cancelled_runs: u64,
    /// Number of distinct actors that have triggered runs.
    pub distinct_actors: u64,
    /// Longest completed run duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_run_duration_ms: Option<u64>,
    /// Average completed run duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_run_duration_ms: Option<f64>,
}
