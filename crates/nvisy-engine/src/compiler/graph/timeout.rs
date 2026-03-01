//! Timeout configuration for pipeline graph nodes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Policy controlling how long a node may run before timing out.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TimeoutPolicy {
    /// Maximum wall-clock time in milliseconds before the node is interrupted.
    pub duration_ms: u64,
    /// What to do when the timeout fires.
    #[serde(default)]
    pub on_timeout: TimeoutBehavior,
}

/// Behaviour when a node exceeds its [`TimeoutPolicy`] deadline.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimeoutBehavior {
    /// Return an error and propagate the failure.
    #[default]
    Fail,
    /// Silently discard the result and report zero items processed.
    Skip,
}
