//! Retry policy types and backoff strategies.
//!
//! [`RetryPolicy`] configures how many times a failed node should be retried,
//! the base delay between attempts, and the [`BackoffStrategy`] to use.

use serde::{Deserialize, Serialize};

/// Retry policy attached to a pipeline node.
///
/// Defaults to 3 retries with a 1 000 ms fixed delay.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts after the initial failure.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Base delay in milliseconds between retry attempts.
    #[serde(default = "default_delay_ms")]
    pub delay_ms: u64,
    /// Strategy used to compute the delay between successive retries.
    #[serde(default)]
    pub backoff: BackoffStrategy,
}

/// Returns the default maximum retry count (3).
fn default_max_retries() -> u32 { 3 }
/// Returns the default base delay in milliseconds (1 000).
fn default_delay_ms() -> u64 { 1000 }

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            delay_ms: 1000,
            backoff: BackoffStrategy::default(),
        }
    }
}

/// Strategy for computing the delay between retry attempts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
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
