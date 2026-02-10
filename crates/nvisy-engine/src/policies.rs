use std::time::Duration;
use tokio::time;
use nvisy_core::errors::NvisyError;
use crate::schema::{BackoffStrategy, RetryPolicy};

/// Compute delay for a retry attempt.
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

/// Execute a future with retry logic.
pub async fn with_retry<F, Fut, T>(
    policy: &RetryPolicy,
    mut f: F,
) -> Result<T, NvisyError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, NvisyError>>,
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
    Err(last_err.unwrap_or_else(|| NvisyError::runtime("Retry exhausted", "policies", false)))
}

/// Execute a future with a timeout.
pub async fn with_timeout<F, T>(
    timeout_ms: u64,
    f: F,
) -> Result<T, NvisyError>
where
    F: std::future::Future<Output = Result<T, NvisyError>>,
{
    match time::timeout(Duration::from_millis(timeout_ms), f).await {
        Ok(result) => result,
        Err(_) => Err(NvisyError::timeout(format!(
            "Operation timed out after {}ms",
            timeout_ms
        ))),
    }
}
