//! User-facing timeout policy configuration.
//!
//! [`TimeoutPolicy`] controls how long a node may run before timing out.

use std::time::Duration;

use nvisy_core::Error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::time;
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

    /// Wraps a future with a deadline, returning an [`Error::timeout`] if it
    /// does not complete within the configured duration.
    pub async fn with_timeout<F, T>(&self, f: F) -> Result<T, Error>
    where
        F: std::future::Future<Output = Result<T, Error>>,
    {
        match time::timeout(self.duration(), f).await {
            Ok(result) => result,
            Err(_) => Err(Error::timeout(format!(
                "Operation timed out after {}ms",
                self.duration_ms,
            ))),
        }
    }

    /// Apply an optional timeout to a future. If no policy is provided, call directly.
    pub async fn call<F, T>(timeout: Option<&Self>, f: F) -> Result<T, Error>
    where
        F: std::future::Future<Output = Result<T, Error>>,
    {
        match timeout {
            Some(policy) => policy.with_timeout(f).await,
            None => f.await,
        }
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
