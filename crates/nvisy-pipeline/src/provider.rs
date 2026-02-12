//! Provider trait for creating authenticated client connections.

use std::future::Future;
use std::pin::Pin;

use serde::de::DeserializeOwned;

use nvisy_core::error::Error;

/// A connected provider instance holding a typed client and an
/// optional async disconnect callback.
pub struct ConnectedInstance<C> {
    /// Typed client handle.
    pub client: C,
    /// Optional cleanup function called when the connection is no longer needed.
    pub disconnect: Option<Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>>,
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
    type Client: Send + 'static;

    /// Unique identifier (e.g. "s3", "openai").
    fn id(&self) -> &str;

    /// Validate credentials shape without connecting.
    fn validate_credentials(&self, creds: &Self::Credentials) -> Result<(), Error>;

    /// Verify credentials by attempting a lightweight connection.
    async fn verify(&self, creds: &Self::Credentials) -> Result<(), Error>;

    /// Create a connected instance.
    async fn connect(
        &self,
        creds: &Self::Credentials,
    ) -> Result<ConnectedInstance<Self::Client>, Error>;
}
