use std::any::Any;
use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;

use crate::errors::NvisyError;

/// A connected provider instance with an opaque client and optional disconnect callback.
pub struct ConnectedInstance {
    pub client: Box<dyn Any + Send>,
    pub disconnect: Option<Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>>,
}

/// Factory for creating connected provider instances.
#[async_trait]
pub trait ProviderFactory: Send + Sync + 'static {
    /// Unique identifier (e.g. "s3", "openai").
    fn id(&self) -> &str;

    /// Validate credentials shape without connecting.
    fn validate_credentials(&self, creds: &serde_json::Value) -> Result<(), NvisyError>;

    /// Verify credentials by attempting a lightweight connection.
    async fn verify(&self, creds: &serde_json::Value) -> Result<(), NvisyError>;

    /// Create a connected instance.
    async fn connect(&self, creds: &serde_json::Value) -> Result<ConnectedInstance, NvisyError>;
}
