//! Abstract object-store client trait and helper types.
//!
//! The [`ObjectStoreClient`] trait defines the CRUD surface that every backend
//! (S3, GCS, local filesystem, etc.) must implement.  [`ObjectStoreBox`] wraps
//! a concrete client so it can be passed through the engine as `Box<dyn Any + Send>`.

use bytes::Bytes;

/// Result returned by [`ObjectStoreClient::list`].
pub struct ListResult {
    /// Object keys matching the requested prefix.
    pub keys: Vec<String>,
    /// Opaque pagination cursor; `None` when there are no more pages.
    pub next_cursor: Option<String>,
}

/// Abstract client for object storage operations.
///
/// Implementations provide list, get, put, and delete over a single bucket
/// or container.
#[async_trait::async_trait]
pub trait ObjectStoreClient: Send + Sync + 'static {
    /// List object keys under `prefix`, optionally continuing from `cursor`.
    async fn list(&self, prefix: &str, cursor: Option<&str>) -> Result<ListResult, Box<dyn std::error::Error + Send + Sync>>;
    /// Retrieve the object stored at `key`.
    async fn get(&self, key: &str) -> Result<GetResult, Box<dyn std::error::Error + Send + Sync>>;
    /// Upload `data` to `key`, optionally setting the content-type header.
    async fn put(&self, key: &str, data: Bytes, content_type: Option<&str>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// Delete the object at `key`.
    async fn delete(&self, key: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Result returned by [`ObjectStoreClient::get`].
pub struct GetResult {
    /// Raw bytes of the retrieved object.
    pub data: Bytes,
    /// MIME content-type, if the backend provides one.
    pub content_type: Option<String>,
}

/// Type-erased wrapper around a boxed [`ObjectStoreClient`].
///
/// This allows the client to be stored as `Box<dyn Any + Send>` inside the
/// engine while still being downcasted back to a usable object-store client.
pub struct ObjectStoreBox(pub Box<dyn ObjectStoreClient>);

impl ObjectStoreBox {
    /// Wrap a concrete [`ObjectStoreClient`] implementation.
    pub fn new(client: impl ObjectStoreClient) -> Self {
        Self(Box::new(client))
    }
}
