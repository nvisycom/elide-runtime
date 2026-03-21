//! Async retry execution for [`RetryPolicy`].

use nvisy_core::Error;
use tokio::time;

use crate::graph::RetryPolicy;

/// Async execution helpers for [`RetryPolicy`].
impl RetryPolicy {
    /// Executes a fallible async closure with automatic retry.
    ///
    /// The closure is invoked up to `max_retries + 1` times. Non-retryable
    /// errors (as determined by [`Error::is_retryable`]) are returned
    /// immediately.
    pub async fn with_retry<F, Fut, T>(&self, mut f: F) -> Result<T, Error>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, Error>>,
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
                    time::sleep(delay).await;
                }
            }
        }
        Err(last_err.unwrap_or_else(|| Error::runtime("Retry exhausted", "policy", false)))
    }

    /// Call a closure with optional retry. If no policy is provided, call directly.
    pub async fn call<T, F, Fut>(retry: Option<&Self>, mut f: F) -> Result<T, Error>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, Error>>,
    {
        match retry {
            Some(policy) => policy.with_retry(f).await,
            None => f().await,
        }
    }
}
