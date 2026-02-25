//! Retry policy with exponential backoff.

use std::future::Future;
use std::time::Duration;

use nvisy_core::Error;

/// Exponential backoff retry policy.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retries (default: 3).
    pub max_retries: u32,
    /// Initial backoff duration (default: 300ms).
    pub initial_backoff: Duration,
    /// Multiplicative backoff factor (default: 2.0).
    pub backoff_factor: f64,
    /// Maximum backoff duration cap (default: 5s).
    pub max_backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl RetryPolicy {
    /// Create a retry policy with default settings.
    pub fn new() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(300),
            backoff_factor: 2.0,
            max_backoff: Duration::from_secs(5),
        }
    }

    /// Execute an async closure with retry on retryable errors.
    pub async fn execute<F, Fut, T>(&self, operation: F) -> Result<T, Error>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, Error>>,
    {
        let mut attempts = 0u32;
        let mut backoff = self.initial_backoff;

        loop {
            match operation().await {
                Ok(val) => return Ok(val),
                Err(err) => {
                    if !err.is_retryable() || attempts >= self.max_retries {
                        return Err(err);
                    }

                    attempts += 1;
                    tracing::warn!(
                        attempt = attempts,
                        max_retries = self.max_retries,
                        backoff_ms = backoff.as_millis() as u64,
                        error = %err,
                        "retrying after transient error"
                    );

                    tokio::time::sleep(backoff).await;

                    backoff = Duration::from_secs_f64(
                        (backoff.as_secs_f64() * self.backoff_factor).min(self.max_backoff.as_secs_f64()),
                    );
                }
            }
        }
    }

    /// Return the number of retries that were consumed during the last
    /// [`execute`](Self::execute) call. This is tracked externally by the
    /// caller; here we just expose a helper to compute attempts from the
    /// backoff state if needed.
    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn succeeds_on_first_try() {
        let policy = RetryPolicy::new();
        let result = policy.execute(|| async { Ok::<_, Error>(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn retries_on_retryable_error() {
        let counter = AtomicU32::new(0);
        let policy = RetryPolicy {
            max_retries: 3,
            initial_backoff: Duration::from_millis(1),
            backoff_factor: 1.0,
            max_backoff: Duration::from_millis(1),
        };

        let result = policy
            .execute(|| {
                let attempt = counter.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt < 2 {
                        Err(Error::connection("transient", "test", true))
                    } else {
                        Ok(42)
                    }
                }
            })
            .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn does_not_retry_non_retryable() {
        let counter = AtomicU32::new(0);
        let policy = RetryPolicy {
            max_retries: 3,
            initial_backoff: Duration::from_millis(1),
            backoff_factor: 1.0,
            max_backoff: Duration::from_millis(1),
        };

        let result: Result<i32, _> = policy
            .execute(|| {
                counter.fetch_add(1, Ordering::SeqCst);
                async { Err(Error::validation("bad input", "test")) }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
