//! Async timeout execution for [`TimeoutPolicy`].

use nvisy_core::Error;
use tokio::time;

use crate::graph::TimeoutPolicy;

/// Async execution helpers for [`TimeoutPolicy`].
impl TimeoutPolicy {
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
