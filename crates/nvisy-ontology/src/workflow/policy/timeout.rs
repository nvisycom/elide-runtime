//! User-facing timeout policy configuration.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Policy controlling how long a node may run before timing out.
#[derive(Debug, Clone, Validate, Serialize, Deserialize, JsonSchema)]
pub struct TimeoutPolicy {
    /// Maximum wall-clock time in milliseconds before the node is interrupted.
    #[validate(range(min = 1, max = 60_000))]
    pub duration_ms: u64,
    /// What to do when the timeout fires.
    #[serde(default)]
    pub on_timeout: TimeoutBehavior,
}

impl TimeoutPolicy {
    /// Returns the timeout duration.
    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.duration_ms)
    }
}

/// Behaviour when a node exceeds its [`TimeoutPolicy`] deadline.
#[derive(Debug, Clone, Copy, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimeoutBehavior {
    /// Return an error and propagate the failure.
    #[default]
    Fail,
    /// Silently discard the result and report zero items processed.
    Skip,
}
