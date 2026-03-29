//! User-facing retry policy configuration.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
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
#[non_exhaustive]
pub enum BackoffStrategy {
    /// Constant delay equal to `delay_ms` on every attempt.
    #[default]
    Fixed,
    /// Delay doubles with each attempt: `delay_ms * 2^attempt`.
    Exponential,
    /// Exponential backoff with an added random jitter to prevent thundering herd.
    Jitter,
}
