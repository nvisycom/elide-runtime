//! Exponential-backoff retry middleware.

use reqwest_retry::RetryTransientMiddleware;
use reqwest_retry::policies::ExponentialBackoff;

/// Build an exponential-backoff policy with the given maximum retries.
pub fn backoff_policy(max_retries: u32) -> ExponentialBackoff {
    ExponentialBackoff::builder().build_with_max_retries(max_retries)
}

/// Wrap a backoff policy into the retry middleware layer.
pub fn layer(policy: ExponentialBackoff) -> RetryTransientMiddleware<ExponentialBackoff> {
    RetryTransientMiddleware::new_with_policy(policy)
}
