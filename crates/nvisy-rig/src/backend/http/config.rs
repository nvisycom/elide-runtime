//! HTTP client configuration.

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
pub struct HttpConfig {
    /// Maximum number of retries for transient failures (default: 3).
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Per-request timeout in seconds (default: 120s).
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// TCP connection timeout in seconds (default: 10s).
    #[serde(default = "default_connect_timeout_secs")]
    pub connect_timeout_secs: u64,
    /// Keep-alive pool idle timeout in seconds (default: 90s).
    #[serde(default = "default_pool_idle_timeout_secs")]
    pub pool_idle_timeout_secs: u64,
}

fn default_max_retries() -> u32 {
    DEFAULT_MAX_RETRIES
}
fn default_timeout_secs() -> u64 {
    DEFAULT_TIMEOUT_SECS
}
fn default_connect_timeout_secs() -> u64 {
    DEFAULT_CONNECT_TIMEOUT_SECS
}
fn default_pool_idle_timeout_secs() -> u64 {
    DEFAULT_POOL_IDLE_TIMEOUT_SECS
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            connect_timeout_secs: DEFAULT_CONNECT_TIMEOUT_SECS,
            pool_idle_timeout_secs: DEFAULT_POOL_IDLE_TIMEOUT_SECS,
        }
    }
}

impl HttpConfig {
    /// Create a config with the given max retries and default timeouts.
    pub fn with_max_retries(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }
}
