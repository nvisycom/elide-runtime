//! User-facing timeout policy configuration.

use std::future::Future;
use std::time::Duration;

use nvisy_core::Error;
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

impl TimeoutPolicy {
    /// Wrap a future with this policy's deadline.
    ///
    /// On timeout, returns [`Error::timeout`] with the configured
    /// duration. The `on_timeout` field's `Skip` variant is not yet
    /// honored here — callers that want skip-on-timeout semantics
    /// should branch on the returned error.
    pub async fn with_timeout<F, T: Send>(&self, f: F) -> Result<T, Error>
    where
        F: Future<Output = Result<T, Error>> + Send,
    {
        match tokio::time::timeout(Duration::from_millis(self.duration_ms), f).await {
            Ok(result) => result,
            Err(_) => Err(Error::timeout(format!(
                "Operation timed out after {}ms",
                self.duration_ms,
            ))),
        }
    }
}
