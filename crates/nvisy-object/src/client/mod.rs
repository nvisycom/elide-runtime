use async_trait::async_trait;
use bytes::Bytes;

/// Result of a list operation.
pub struct ListResult {
    pub keys: Vec<String>,
    pub next_cursor: Option<String>,
}

/// Abstract client for object storage operations.
#[async_trait]
pub trait ObjectStoreClient: Send + Sync + 'static {
    async fn list(&self, prefix: &str, cursor: Option<&str>) -> Result<ListResult, Box<dyn std::error::Error + Send + Sync>>;
    async fn get(&self, key: &str) -> Result<GetResult, Box<dyn std::error::Error + Send + Sync>>;
    async fn put(&self, key: &str, data: Bytes, content_type: Option<&str>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn delete(&self, key: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Result of a get operation.
pub struct GetResult {
    pub data: Bytes,
    pub content_type: Option<String>,
}

/// A sized wrapper around a boxed ObjectStoreClient, usable with `Box<dyn Any + Send>`.
pub struct ObjectStoreBox(pub Box<dyn ObjectStoreClient>);

impl ObjectStoreBox {
    pub fn new(client: impl ObjectStoreClient) -> Self {
        Self(Box::new(client))
    }
}
