//! HTTP client construction with retry and tracing middleware.

use std::time::Duration;

use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::RetryTransientMiddleware;
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_tracing::TracingMiddleware;

use super::HttpConfig;

const TARGET: &str = "nvisy_ocr::http";

/// Build a [`ClientWithMiddleware`] from the given configuration.
///
/// The returned client has exponential-backoff retry and OpenTelemetry
/// tracing middleware pre-installed.
pub fn build_http_client(config: &HttpConfig) -> ClientWithMiddleware {
    tracing::debug!(
        target: TARGET,
        max_retries = config.max_retries,
        timeout_secs = config.timeout_secs,
        connect_timeout_secs = config.connect_timeout_secs,
        pool_idle_timeout_secs = config.pool_idle_timeout_secs,
        "building HTTP client"
    );

    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(config.max_retries);

    let client = reqwest_middleware::reqwest::Client::builder()
        .timeout(Duration::from_secs(config.timeout_secs))
        .connect_timeout(Duration::from_secs(config.connect_timeout_secs))
        .pool_idle_timeout(Duration::from_secs(config.pool_idle_timeout_secs))
        .build()
        .expect("failed to build reqwest client");

    ClientBuilder::new(client)
        .with(TracingMiddleware::default())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}
