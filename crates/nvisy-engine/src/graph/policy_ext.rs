//! Async execution extensions for [`RetryPolicy`] and [`TimeoutPolicy`].
//!
//! The policy structs are pure data defined in [`nvisy_ontology::graph`].
//! This module adds the tokio-dependent execution methods via traits.

use nvisy_core::Error;
use nvisy_ontology::graph::{RetryPolicy, TimeoutPolicy};

/// Async retry execution for [`RetryPolicy`].
pub(crate) trait RetryExt {
    /// Executes a fallible async closure with automatic retry.
    fn with_retry<F, Fut, T: Send>(
        &self,
        f: F,
    ) -> impl std::future::Future<Output = Result<T, Error>> + Send
    where
        F: FnMut() -> Fut + Send,
        Fut: std::future::Future<Output = Result<T, Error>> + Send;
}

impl RetryExt for RetryPolicy {
    async fn with_retry<F, Fut, T: Send>(&self, mut f: F) -> Result<T, Error>
    where
        F: FnMut() -> Fut + Send,
        Fut: std::future::Future<Output = Result<T, Error>> + Send,
    {
        let mut last_err = None;
        for attempt in 0..=self.max_retries {
            match f().await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    if !e.is_retryable() || attempt == self.max_retries {
                        return Err(e);
                    }
                    last_err = Some(e);
                    let delay = self.compute_delay(attempt);
                    tokio::time::sleep(delay).await;
                }
            }
        }
        Err(last_err.unwrap_or_else(|| Error::runtime("Retry exhausted", "policy", false)))
    }
}

/// Async timeout execution for [`TimeoutPolicy`].
pub(crate) trait TimeoutExt {
    /// Wraps a future with a deadline.
    fn with_timeout<F, T: Send>(
        &self,
        f: F,
    ) -> impl std::future::Future<Output = Result<T, Error>> + Send
    where
        F: std::future::Future<Output = Result<T, Error>> + Send;
}

impl TimeoutExt for TimeoutPolicy {
    async fn with_timeout<F, T: Send>(&self, f: F) -> Result<T, Error>
    where
        F: std::future::Future<Output = Result<T, Error>> + Send,
    {
        match tokio::time::timeout(self.duration(), f).await {
            Ok(result) => result,
            Err(_) => Err(Error::timeout(format!(
                "Operation timed out after {}ms",
                self.duration_ms,
            ))),
        }
    }
}
