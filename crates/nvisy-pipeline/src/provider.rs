//! Provider trait for creating authenticated client connections.

use std::future::Future;
use std::pin::Pin;

use serde::de::DeserializeOwned;

use nvisy_core::error::Error;

/// Factory for creating authenticated connections to an external service.
///
/// Implementations handle credential validation, connectivity verification,
/// and client construction for a specific provider (e.g. S3, OpenAI).
#[async_trait::async_trait]
pub trait Provider: Send + Sync + 'static {
    /// Strongly-typed credentials for this provider.
    type Credentials: DeserializeOwned + Send;
    /// The client type produced by [`connect`](Self::connect).
    type Client: Send + 'static;

    /// Unique identifier (e.g. "s3", "openai").
    fn id(&self) -> &str;

    /// Verify credentials by attempting a lightweight connection.
    async fn verify(creds: &Self::Credentials) -> Result<(), Error>;

    /// Create a connected client instance.
    async fn connect(creds: &Self::Credentials) -> Result<Self::Client, Error>;

    /// Optional async cleanup when the connection is released.
    ///
    /// Return `None` if no cleanup is needed. The default implementation
    /// returns `None`.
    #[allow(clippy::type_complexity)]
    fn disconnect(_client: Self::Client) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
        None
    }
}
