//! Compiled timeout policy with pre-computed duration and async execution helper.

use std::time::Duration;

use nvisy_core::Error;
use tokio::time;

use crate::graph::policy::{TimeoutBehavior, TimeoutPolicy};

/// Pre-compiled timeout policy ready for runtime use.
///
/// Converts the user-facing [`TimeoutPolicy`] (raw milliseconds) into a
/// runtime representation with a [`Duration`] deadline.
#[derive(Debug, Clone)]
pub struct CompiledTimeoutPolicy {
    /// Maximum wall-clock time before the node is interrupted.
    pub duration: Duration,
    /// What to do when the timeout fires.
    pub on_timeout: TimeoutBehavior,
}

impl From<&TimeoutPolicy> for CompiledTimeoutPolicy {
    fn from(policy: &TimeoutPolicy) -> Self {
        Self {
            duration: Duration::from_millis(policy.duration_ms),
            on_timeout: policy.on_timeout,
        }
    }
}

impl CompiledTimeoutPolicy {
    /// Wraps a future with a deadline, returning an [`Error::timeout`] if it
    /// does not complete within the configured duration.
    pub async fn with_timeout<F, T>(&self, f: F) -> Result<T, Error>
    where
        F: std::future::Future<Output = Result<T, Error>>,
    {
        match time::timeout(self.duration, f).await {
            Ok(result) => result,
            Err(_) => Err(Error::timeout(format!(
                "Operation timed out after {}ms",
                self.duration.as_millis(),
            ))),
        }
    }
}
