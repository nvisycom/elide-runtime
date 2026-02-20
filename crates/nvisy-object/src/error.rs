//! Error types for object store operations.

/// Errors produced by [`ObjectStoreClient`] and provider factories.
///
/// [`ObjectStoreClient`]: crate::client::ObjectStoreClient
#[derive(Debug, thiserror::Error)]
pub enum ObjectStoreError {
    /// Failure from the underlying [`object_store`] backend.
    #[error(transparent)]
    Store(#[from] object_store::Error),

    /// Provider failed to build a client from credentials.
    #[error("provider `{provider}`: {message}")]
    Connect {
        provider: &'static str,
        message: String,
    },
}

impl ObjectStoreError {
    /// Create a connection error for the given provider.
    pub fn connect(provider: &'static str, err: impl std::fmt::Display) -> Self {
        Self::Connect {
            provider,
            message: err.to_string(),
        }
    }
}

impl From<ObjectStoreError> for nvisy_core::error::Error {
    fn from(err: ObjectStoreError) -> Self {
        match &err {
            ObjectStoreError::Store(_) => {
                nvisy_core::error::Error::runtime(err.to_string(), "object-store", true)
            }
            ObjectStoreError::Connect { provider, .. } => {
                nvisy_core::error::Error::connection(err.to_string(), *provider, true)
            }
        }
    }
}
