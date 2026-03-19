//! Compiled retry policy with pre-computed delay and async execution helper.

use std::time::Duration;

use nvisy_core::Error;
use tokio::time;

use crate::graph::{BackoffStrategy, RetryPolicy};

/// Pre-compiled retry policy ready for runtime use.
///
/// Converts the user-facing [`RetryPolicy`] (raw milliseconds) into a
/// runtime representation with a [`Duration`] base delay.
#[derive(Debug, Clone)]
pub struct CompiledRetryPolicy {
    /// Maximum number of retry attempts after the initial failure.
    pub max_retries: u32,
    /// Base delay between retry attempts.
    pub base_delay: Duration,
    /// Strategy used to compute the delay between successive retries.
    pub backoff: BackoffStrategy,
}

impl From<&RetryPolicy> for CompiledRetryPolicy {
    fn from(policy: &RetryPolicy) -> Self {
        Self {
            max_retries: policy.max_retries,
            base_delay: Duration::from_millis(policy.delay_ms),
            backoff: policy.backoff,
        }
    }
}

impl CompiledRetryPolicy {
    /// Computes the sleep duration for a given zero-based attempt number.
    pub fn compute_delay(&self, attempt: u32) -> Duration {
        match self.backoff {
            BackoffStrategy::Fixed => self.base_delay,
            BackoffStrategy::Exponential => self.base_delay * 2u32.saturating_pow(attempt),
            BackoffStrategy::Jitter => {
                let exp = self.base_delay * 2u32.saturating_pow(attempt);
                let jitter_range = exp.as_millis() as u64 + 1;
                let jitter = Duration::from_millis(rand::random_range(0..jitter_range));
                exp + jitter
            }
        }
    }

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
    pub async fn call<T, F, Fut>(
        retry: Option<&Self>,
        mut f: F,
    ) -> Result<T, Error>
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
