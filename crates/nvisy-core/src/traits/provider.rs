//! Provider factory trait for creating authenticated client connections.

use std::any::Any;
use std::future::Future;
use std::pin::Pin;

use crate::error::Error;

/// A connected provider instance holding an opaque client and an
/// optional async disconnect callback.
///
/// The `client` is type-erased so that different providers (S3, OpenAI,
/// databases, etc.) can return their own client types without requiring
/// a common interface.
pub struct ConnectedInstance {
    /// Type-erased client handle, downcast by consumers to the concrete type.
    pub client: Box<dyn Any + Send>,
    /// Optional cleanup function called when the connection is no longer needed.
    pub disconnect: Option<Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>>,
}

/// Factory for creating authenticated connections to an external service.
///
/// Implementations handle credential validation, connectivity verification,
/// and client construction for a specific provider (e.g. S3, OpenAI).
#[async_trait::async_trait]
pub trait ProviderFactory: Send + Sync + 'static {
    /// Unique identifier (e.g. "s3", "openai").
    fn id(&self) -> &str;

    /// Validate credentials shape without connecting.
    fn validate_credentials(&self, creds: &serde_json::Value) -> Result<(), Error>;

    /// Verify credentials by attempting a lightweight connection.
    async fn verify(&self, creds: &serde_json::Value) -> Result<(), Error>;

    /// Create a connected instance.
    async fn connect(&self, creds: &serde_json::Value) -> Result<ConnectedInstance, Error>;
}
