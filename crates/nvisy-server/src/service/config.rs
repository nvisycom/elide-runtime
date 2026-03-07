//! Service configuration for registry and engine defaults.

use std::path::PathBuf;

use clap::Args;
use nvisy_engine::{RetryPolicy, TimeoutPolicy};
use nvisy_http::HttpConfig;

/// Configuration for the service layer.
///
/// Controls the registry data directory and default engine policies.
/// Can be flattened into a parent CLI struct via `#[command(flatten)]`.
#[derive(Debug, Clone, Args)]
pub struct ServiceConfig {
    /// Directory for data storage (content, contexts).
    ///
    /// Defaults to `$TMPDIR/nvisy-server-data` if not set.
    #[arg(long, env = "DATA_DIR")]
    data_dir: Option<PathBuf>,

    /// Default retry max attempts for graph nodes (0 to disable).
    #[arg(long, env = "ENGINE_RETRY_MAX", default_value_t = 3)]
    engine_retry_max: u32,

    /// Default retry base delay in milliseconds.
    #[arg(long, env = "ENGINE_RETRY_DELAY_MS", default_value_t = 500)]
    engine_retry_delay_ms: u64,

    /// Default timeout in milliseconds for graph nodes (0 to disable).
    #[arg(long, env = "ENGINE_TIMEOUT_MS", default_value_t = 30_000)]
    engine_timeout_ms: u64,

    /// Maximum retries for transient HTTP errors in downstream providers.
    #[arg(long, env = "HTTP_MAX_RETRIES", default_value_t = 3)]
    http_max_retries: u32,

    /// Per-request timeout in seconds for downstream HTTP calls.
    #[arg(long, env = "HTTP_TIMEOUT_SECS", default_value_t = 120)]
    http_timeout_secs: u64,

    /// TCP connection timeout in seconds for downstream HTTP calls.
    #[arg(long, env = "HTTP_CONNECT_TIMEOUT_SECS", default_value_t = 10)]
    http_connect_timeout_secs: u64,

    /// Keep-alive pool idle timeout in seconds for downstream HTTP calls.
    #[arg(long, env = "HTTP_POOL_IDLE_TIMEOUT_SECS", default_value_t = 90)]
    http_pool_idle_timeout_secs: u64,
}

impl ServiceConfig {
    /// Returns the data directory, falling back to a temp directory.
    pub fn data_dir(&self) -> PathBuf {
        self.data_dir
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join("nvisy-server-data"))
    }

    /// Builds the default retry policy, if retries are enabled.
    pub fn retry_policy(&self) -> Option<RetryPolicy> {
        if self.engine_retry_max == 0 {
            return None;
        }
        Some(RetryPolicy {
            max_retries: self.engine_retry_max,
            delay_ms: self.engine_retry_delay_ms,
            backoff: Default::default(),
        })
    }

    /// Builds the default timeout policy, if a timeout is set.
    pub fn timeout_policy(&self) -> Option<TimeoutPolicy> {
        if self.engine_timeout_ms == 0 {
            return None;
        }
        Some(TimeoutPolicy {
            duration_ms: self.engine_timeout_ms,
            on_timeout: Default::default(),
        })
    }

    /// Builds the HTTP configuration for downstream provider clients.
    pub fn http_config(&self) -> HttpConfig {
        HttpConfig {
            max_retries: self.http_max_retries,
            timeout_secs: self.http_timeout_secs,
            connect_timeout_secs: self.http_connect_timeout_secs,
            pool_idle_timeout_secs: self.http_pool_idle_timeout_secs,
        }
    }
}
