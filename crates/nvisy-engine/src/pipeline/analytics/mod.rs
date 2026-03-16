//! Aggregate pipeline analytics types and the [`Analytics`] trait.
//!
//! Pure data definitions for pipeline-wide metrics. Querying happens
//! through the [`Analytics`] trait, implemented on
//! [`DefaultEngine`](super::DefaultEngine).

use std::future::Future;

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
    /// Total number of entities detected across all runs.
    pub total_entities_detected: u64,
    /// Total number of redactions applied across all runs.
    pub total_redactions_applied: u64,
}

/// Read-only access to aggregate pipeline analytics.
pub trait EngineAnalytics: Send + Sync {
    /// Collect a point-in-time analytics snapshot.
    fn snapshot(&self) -> impl Future<Output = AnalyticsSnapshot> + Send;
}
