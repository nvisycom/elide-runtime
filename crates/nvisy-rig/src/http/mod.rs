//! HTTP client construction with retry and tracing middleware.

mod config;
mod middleware;

use std::fmt;

use derive_more::Deref;
use nvisy_core::{Error, Result};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware, RequestBuilder};

pub use self::config::HttpConfig;
use self::middleware::{backoff_policy, retry_layer, tracing_layer};

const TARGET: &str = "nvisy_rig::http";

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
    pub fn new(config: &HttpConfig) -> Result<Self> {
        tracing::debug!(
            target: TARGET,
            max_retries = config.max_retries,
            timeout = ?config.timeout,
            connect_timeout = ?config.connect_timeout,
            idle_timeout = ?config.idle_timeout,
            "building HTTP client"
        );

        let policy = backoff_policy(config.max_retries);

        let client = reqwest_middleware::reqwest::Client::builder()
            .timeout(config.timeout)
            .connect_timeout(config.connect_timeout)
            .pool_idle_timeout(config.idle_timeout)
            .build()
            .map_err(|e| {
                Error::runtime(format!("failed to build HTTP client: {e}"), "http", false)
            })?;

        Ok(Self(
            ClientBuilder::new(client)
                .with(tracing_layer())
                .with(retry_layer(policy))
                .build(),
        ))
    }

    /// Consume the wrapper and return the inner middleware client.
    pub fn into_inner(self) -> ClientWithMiddleware {
        self.0
    }
}

impl fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    ) -> impl Future<Output = Result<reqwest_middleware::reqwest::Response>> + Send;

    /// Send the request, check status, and parse the JSON response body.
    fn send_and_parse<T: serde::de::DeserializeOwned>(
        self,
        provider: &str,
    ) -> impl Future<Output = Result<T>> + Send;
}

impl RequestBuilderExt for RequestBuilder {
    async fn send_and_check(self, provider: &str) -> Result<reqwest_middleware::reqwest::Response> {
        let resp = self
            .send()
            .await
            .map_err(|e| Error::connection(e.to_string(), provider, true))?;

        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let body = resp.text().await.unwrap_or_default();
        Err(Error::connection(
            format!("{provider} returned {status}: {body}"),
            provider,
            status.is_server_error(),
        ))
    }

    async fn send_and_parse<T: serde::de::DeserializeOwned>(self, provider: &str) -> Result<T> {
        let resp = self.send_and_check(provider).await?;
        resp.json().await.map_err(|e| {
            Error::runtime(format!("{provider} JSON parse error: {e}"), provider, false)
        })
    }
}
