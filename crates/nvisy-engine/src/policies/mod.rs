//! Retry and timeout policies for pipeline execution.
//!
//! Provides [`compute_delay`] for backoff calculation, [`with_retry`] for
//! automatic retry of fallible futures, and [`with_timeout`] for deadline
//! enforcement.

use std::time::Duration;
use tokio::time;
use nvisy_core::error::Error;
pub mod retry;

use crate::policies::retry::{BackoffStrategy, RetryPolicy};

/// Computes the sleep duration before a retry attempt based on the policy's
/// [`BackoffStrategy`] and the zero-based attempt number.
pub fn compute_delay(policy: &RetryPolicy, attempt: u32) -> Duration {
    let base = Duration::from_millis(policy.delay_ms);
    match policy.backoff {
        BackoffStrategy::Fixed => base,
        BackoffStrategy::Exponential => base * 2u32.saturating_pow(attempt),
        BackoffStrategy::Jitter => {
            let exp = base * 2u32.saturating_pow(attempt);
            let jitter_range = exp.as_millis() as u64 + 1;
            let jitter = Duration::from_millis(rand::random_range(0..jitter_range));
            exp + jitter
        }
    }
}

/// Executes a fallible async closure with automatic retry according to the
/// given [`RetryPolicy`].
///
/// The closure is invoked up to `max_retries + 1` times. Non-retryable errors
/// (as determined by [`Error::is_retryable`]) are returned immediately.
pub async fn with_retry<F, Fut, T>(
    policy: &RetryPolicy,
    mut f: F,
) -> Result<T, Error>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, Error>>,
{
    let mut last_err = None;
    for attempt in 0..=policy.max_retries {
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                if !e.is_retryable() || attempt == policy.max_retries {
                    return Err(e);
                }
                last_err = Some(e);
                let delay = compute_delay(policy, attempt);
                time::sleep(delay).await;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| Error::runtime("Retry exhausted", "policies", false)))
}

/// Wraps a future with a deadline, returning an [`Error::timeout`] if it
/// does not complete within `timeout_ms` milliseconds.
pub async fn with_timeout<F, T>(
    timeout_ms: u64,
    f: F,
) -> Result<T, Error>
where
    F: std::future::Future<Output = Result<T, Error>>,
{
    match time::timeout(Duration::from_millis(timeout_ms), f).await {
        Ok(result) => result,
        Err(_) => Err(Error::timeout(format!(
            "Operation timed out after {}ms",
            timeout_ms
        ))),
    }
}
