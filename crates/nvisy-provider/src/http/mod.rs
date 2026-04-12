//! HTTP client construction with retry and tracing middleware.

mod config;
mod middleware;

use std::time::Duration;

use derive_more::Deref;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware, RequestBuilder};

pub use self::config::HttpConfig;
use self::middleware::{retry, tracing as mw_tracing};

const TARGET: &str = "nvisy_provider::http";

/// Newtype around [`ClientWithMiddleware`] with a [`Default`] implementation
/// that builds a client from [`HttpConfig::default`].
#[derive(Clone, Deref)]
pub struct HttpClient(ClientWithMiddleware);

impl HttpClient {
    /// Build an [`HttpClient`] from the given configuration.
    ///
    /// The returned client has exponential-backoff retry and OpenTelemetry
    /// tracing middleware pre-installed.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying `reqwest::Client` cannot be built
    /// (e.g. TLS backend initialisation failure).
    pub fn new(config: &HttpConfig) -> nvisy_core::Result<Self> {
        tracing::debug!(
            target: TARGET,
            max_retries = config.max_retries,
            timeout_secs = config.timeout_secs,
            connect_timeout_secs = config.connect_timeout_secs,
            idle_timeout_secs = config.idle_timeout_secs,
            "building HTTP client"
        );

        let retry_policy = retry::backoff_policy(config.max_retries);

        let client = reqwest_middleware::reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .connect_timeout(Duration::from_secs(config.connect_timeout_secs))
            .pool_idle_timeout(Duration::from_secs(config.idle_timeout_secs))
            .build()
            .map_err(|e| {
                nvisy_core::Error::runtime(
                    format!("failed to build HTTP client: {e}"),
                    "http",
                    false,
                )
            })?;

        Ok(Self(
            ClientBuilder::new(client)
                .with(mw_tracing::layer())
                .with(retry::layer(retry_policy))
                .build(),
        ))
    }

    /// Consume the wrapper and return the inner middleware client.
    pub fn into_inner(self) -> ClientWithMiddleware {
        self.0
    }
}

impl std::fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("HttpClient").finish()
    }
}

/// Extension trait for [`RequestBuilder`] that adds send + check + parse
/// helpers with standardised error mapping.
///
/// Import this trait to use `.send_and_check("provider")` and
/// `.send_and_parse::<T>("provider")` on any request builder.
pub trait RequestBuilderExt {
    /// Send the request and check the response status.
    ///
    /// Maps transport errors to retryable connection errors and
    /// non-success status codes to connection errors (5xx are retryable).
    fn send_and_check(
        self,
        provider: &str,
    ) -> impl Future<Output = nvisy_core::Result<reqwest_middleware::reqwest::Response>> + Send;

    /// Send the request, check status, and parse the JSON response body.
    fn send_and_parse<T: serde::de::DeserializeOwned>(
        self,
        provider: &str,
    ) -> impl Future<Output = nvisy_core::Result<T>> + Send;
}

impl RequestBuilderExt for RequestBuilder {
    async fn send_and_check(
        self,
        provider: &str,
    ) -> nvisy_core::Result<reqwest_middleware::reqwest::Response> {
        let resp = self
            .send()
            .await
            .map_err(|e| nvisy_core::Error::connection(e.to_string(), provider, true))?;

        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let body = resp.text().await.unwrap_or_default();
        Err(nvisy_core::Error::connection(
            format!("{provider} returned {status}: {body}"),
            provider,
            status.is_server_error(),
        ))
    }

    async fn send_and_parse<T: serde::de::DeserializeOwned>(
        self,
        provider: &str,
    ) -> nvisy_core::Result<T> {
        let resp = self.send_and_check(provider).await?;
        resp.json().await.map_err(|e| {
            nvisy_core::Error::runtime(format!("{provider} JSON parse error: {e}"), provider, false)
        })
    }
}
