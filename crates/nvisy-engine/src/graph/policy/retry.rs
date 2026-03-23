//! User-facing retry policy configuration.

use std::time::Duration;

use nvisy_core::Error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::time;
use validator::Validate;

/// Retry policy attached to a pipeline node.
#[derive(Debug, Clone, Validate, Serialize, Deserialize, JsonSchema)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts after the initial failure.
    #[serde(default = "default_max_retries")]
    #[validate(range(min = 0, max = 5))]
    pub max_retries: u32,
    /// Base delay in milliseconds between retry attempts.
    #[serde(default = "default_delay_ms")]
    #[validate(range(min = 1, max = 30_000))]
    pub delay_ms: u64,
    /// Strategy used to compute the delay between successive retries.
    #[serde(default)]
    pub backoff: BackoffStrategy,
}

impl RetryPolicy {
    /// Base delay as a [`Duration`].
    pub fn base_delay(&self) -> Duration {
        Duration::from_millis(self.delay_ms)
    }

    /// Computes the sleep duration for a given zero-based attempt number.
    pub fn compute_delay(&self, attempt: u32) -> Duration {
        let base = self.base_delay();
        match self.backoff {
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

fn default_max_retries() -> u32 {
    3
}
fn default_delay_ms() -> u64 {
    1000
}

/// Strategy for computing the delay between retry attempts.
#[derive(Debug, Clone, Copy, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackoffStrategy {
    /// Constant delay equal to `delay_ms` on every attempt.
    #[default]
    Fixed,
    /// Delay doubles with each attempt: `delay_ms * 2^attempt`.
    Exponential,
    /// Exponential backoff with an added random jitter to prevent thundering herd.
    Jitter,
}
