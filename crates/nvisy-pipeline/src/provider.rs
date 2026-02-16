//! Provider trait for creating authenticated client connections.

use std::future::Future;
use std::pin::Pin;

use serde::de::DeserializeOwned;

use nvisy_core::error::Error;

/// Implemented by provider clients that support lifecycle management.
///
/// The [`disconnect`](ConnectedInstance::disconnect) method is called when
/// the connection is no longer needed. Implementations that hold no
/// resources can simply return `None`.
pub trait ConnectedInstance: Send + 'static {
    /// Optional async cleanup when the connection is released.
    ///
    /// Return `None` if no cleanup is needed.
    #[allow(clippy::type_complexity)]
    fn disconnect(self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>>;
}

/// Factory for creating authenticated connections to an external service.
///
/// Implementations handle credential validation, connectivity verification,
/// and client construction for a specific provider (e.g. S3, OpenAI).
#[async_trait::async_trait]
pub trait Provider: Send + Sync + 'static {
    /// Strongly-typed credentials for this provider.
    type Credentials: DeserializeOwned + Send;
    /// The client type produced by [`connect`](Self::connect).
    type Client: ConnectedInstance;

    /// Unique identifier (e.g. "s3", "openai").
    fn id(&self) -> &str;

    /// Validate credentials shape without connecting.
    fn validate_credentials(&self, creds: &Self::Credentials) -> Result<(), Error>;

    /// Verify credentials by attempting a lightweight connection.
    async fn verify(&self, creds: &Self::Credentials) -> Result<(), Error>;

    /// Create a connected client instance.
    async fn connect(
        &self,
        creds: &Self::Credentials,
    ) -> Result<Self::Client, Error>;
}
