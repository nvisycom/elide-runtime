//! Unified object-store client backed by [`object_store::ObjectStore`].
//!
//! [`ObjectStoreClient`] is a thin, cloneable wrapper around
//! `Arc<dyn ObjectStore>` that provides convenience methods for the most
//! common operations.

use std::sync::Arc;

use bytes::Bytes;
use object_store::path::Path;
use object_store::{ObjectStore, PutOptions, PutPayload};

use crate::error::ObjectStoreError;

/// Result of a successful [`ObjectStoreClient::get`] call.
pub struct GetResult {
    /// Raw bytes of the retrieved object.
    pub data: Bytes,
    /// MIME content-type, if the backend provides one.
    pub content_type: Option<String>,
}

/// Cloneable handle to any [`ObjectStore`] backend (S3, Azure, GCS, ...).
///
/// All methods accept human-readable string keys and convert them to
/// [`object_store::path::Path`] internally.
#[derive(Clone, Debug)]
pub struct ObjectStoreClient(Arc<dyn ObjectStore>);

impl ObjectStoreClient {
    /// Wrap a concrete [`ObjectStore`] implementation.
    pub fn new(store: impl ObjectStore) -> Self {
        Self(Arc::new(store))
    }

    /// Wrap an already-arced store.
    pub fn from_arc(store: Arc<dyn ObjectStore>) -> Self {
        Self(store)
    }

    /// List object keys under `prefix`.
    ///
    /// Returns all matching keys in a single `Vec`. For very large listings
    /// callers should use the underlying [`ObjectStore::list`] stream directly.
    pub async fn list(
        &self,
        prefix: &str,
    ) -> Result<Vec<object_store::ObjectMeta>, ObjectStoreError> {
        use futures::TryStreamExt;
        let path = Path::from(prefix);
        Ok(self.0.list(Some(&path)).try_collect().await?)
    }

    /// Retrieve the raw bytes and content-type stored at `key`.
    pub async fn get(&self, key: &str) -> Result<GetResult, ObjectStoreError> {
        let path = Path::from(key);
        let result = self.0.get(&path).await?;
        let content_type = result
            .attributes
            .get(&object_store::Attribute::ContentType)
            .map(|v| v.to_string());
        let data = result.bytes().await?;
        Ok(GetResult { data, content_type })
    }

    /// Upload `data` to `key`, optionally setting the content-type.
    pub async fn put(
        &self,
        key: &str,
        data: Bytes,
        content_type: Option<&str>,
    ) -> Result<(), ObjectStoreError> {
        let path = Path::from(key);
        let payload = PutPayload::from(data);
        let mut opts = PutOptions::default();
        if let Some(ct) = content_type {
            opts.attributes.insert(
                object_store::Attribute::ContentType,
                ct.to_string().into(),
            );
        }
        self.0.put_opts(&path, payload, opts).await?;
        Ok(())
    }

    /// Delete the object at `key`.
    pub async fn delete(&self, key: &str) -> Result<(), ObjectStoreError> {
        let path = Path::from(key);
        self.0.delete(&path).await?;
        Ok(())
    }
}
