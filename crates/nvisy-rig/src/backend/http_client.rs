//! Shared HTTP client with timeout, retry, and tracing middleware.

use std::time::Duration;

use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use reqwest_tracing::TracingMiddleware;
use serde::{Deserialize, Serialize};

/// Default request timeout.
const DEFAULT_TIMEOUT_SECS: u64 = 120;
/// Default connection timeout.
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;
/// Default maximum number of retries.
const DEFAULT_MAX_RETRIES: u32 = 3;
/// Default pool idle timeout.
const DEFAULT_POOL_IDLE_TIMEOUT_SECS: u64 = 90;

/// Configuration for the shared HTTP client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpClientConfig {
    /// Maximum number of retries for transient failures.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Per-request timeout in seconds.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// TCP connection timeout in seconds.
    #[serde(default = "default_connect_timeout_secs")]
    pub connect_timeout_secs: u64,
    /// Keep-alive pool idle timeout in seconds.
    #[serde(default = "default_pool_idle_timeout_secs")]
    pub pool_idle_timeout_secs: u64,
}

fn default_max_retries() -> u32 { DEFAULT_MAX_RETRIES }
fn default_timeout_secs() -> u64 { DEFAULT_TIMEOUT_SECS }
fn default_connect_timeout_secs() -> u64 { DEFAULT_CONNECT_TIMEOUT_SECS }
fn default_pool_idle_timeout_secs() -> u64 { DEFAULT_POOL_IDLE_TIMEOUT_SECS }

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            connect_timeout_secs: DEFAULT_CONNECT_TIMEOUT_SECS,
            pool_idle_timeout_secs: DEFAULT_POOL_IDLE_TIMEOUT_SECS,
        }
    }
}

impl HttpClientConfig {
    /// Create a config with the given max retries and default timeouts.
    pub fn with_max_retries(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }
}

/// Build a [`ClientWithMiddleware`] from the given configuration.
pub(crate) fn build_http_client(config: &HttpClientConfig) -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder()
        .build_with_max_retries(config.max_retries);

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
