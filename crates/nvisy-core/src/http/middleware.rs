//! HTTP middleware layers: exponential-backoff retry +
//! OpenTelemetry tracing.

use reqwest_retry::RetryTransientMiddleware;
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_tracing::{DefaultSpanBackend, TracingMiddleware};

/// Build an exponential-backoff policy with the given maximum retries.
pub fn backoff_policy(max_retries: u32) -> ExponentialBackoff {
    ExponentialBackoff::builder().build_with_max_retries(max_retries)
}

/// Wrap a backoff policy into the retry middleware layer.
pub fn retry_layer(policy: ExponentialBackoff) -> RetryTransientMiddleware<ExponentialBackoff> {
    RetryTransientMiddleware::new_with_policy(policy)
}

/// Create the default tracing middleware layer.
pub fn tracing_layer() -> TracingMiddleware<DefaultSpanBackend> {
    TracingMiddleware::default()
}
