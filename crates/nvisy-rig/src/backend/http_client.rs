//! Shared HTTP client with timeout, retry, and tracing middleware.

use std::time::Duration;

use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use reqwest_tracing::TracingMiddleware;

/// Build a `ClientWithMiddleware` with timeout, retry, and tracing middleware.
pub(crate) fn build_http_client(max_retries: u32) -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder()
        .build_with_max_retries(max_retries);

    let client = reqwest_middleware::reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .expect("failed to build reqwest client");

    ClientBuilder::new(client)
        .with(TracingMiddleware::default())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}
