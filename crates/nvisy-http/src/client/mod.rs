//! HTTP client construction with retry and tracing middleware.

mod config;

use std::time::Duration;

use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};

pub use self::config::HttpConfig;
use crate::middleware::{retry, tracing as mw_tracing};

const TARGET: &str = "nvisy_http::client";

/// Newtype around [`ClientWithMiddleware`] with a [`Default`] implementation
/// that builds a client from [`HttpConfig::default`].
#[derive(Clone)]
pub struct HttpClient(ClientWithMiddleware);

impl HttpClient {
    /// Build an [`HttpClient`] from the given configuration.
    ///
    /// The returned client has exponential-backoff retry and OpenTelemetry
    /// tracing middleware pre-installed.
    pub fn new(config: &HttpConfig) -> Self {
        tracing::debug!(
            target: TARGET,
            max_retries = config.max_retries,
            timeout_secs = config.timeout_secs,
            connect_timeout_secs = config.connect_timeout_secs,
            pool_idle_timeout_secs = config.pool_idle_timeout_secs,
            "building HTTP client"
        );

        let retry_policy = retry::backoff_policy(config.max_retries);

        let client = reqwest_middleware::reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .connect_timeout(Duration::from_secs(config.connect_timeout_secs))
            .pool_idle_timeout(Duration::from_secs(config.pool_idle_timeout_secs))
            .build()
            .expect("failed to build reqwest client");

        Self(
            ClientBuilder::new(client)
                .with(mw_tracing::layer())
                .with(retry::layer(retry_policy))
                .build(),
        )
    }

    /// Consume the wrapper and return the inner middleware client.
    pub fn into_inner(self) -> ClientWithMiddleware {
        self.0
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new(&HttpConfig::default())
    }
}

impl std::fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("HttpClient").finish()
    }
}

impl std::ops::Deref for HttpClient {
    type Target = ClientWithMiddleware;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
