//! Unified object-store client backed by [`object_store::ObjectStore`].
//!
//! [`ObjectStoreClient`] is a thin, cloneable wrapper around
//! `Arc<dyn ObjectStore>` that provides convenience methods for the most
//! common operations. Every public method is instrumented with
//! [`tracing`] for observability.

use std::sync::Arc;

use bytes::Bytes;
use futures::stream::BoxStream;
use futures::TryStreamExt;
use object_store::path::Path;
use object_store::{ObjectMeta, ObjectStore, PutMode, PutOptions, PutPayload};

use nvisy_core::Error;

mod get_result;
mod put_result;

pub use get_result::GetResult;
pub use put_result::PutResult;

/// Cloneable handle to any [`ObjectStore`] backend (S3, Azure, GCS, ...).
///
/// All methods accept human-readable string keys and convert them to
/// [`object_store::path::Path`] internally.
#[derive(Clone, Debug)]
pub struct ObjectStoreClient(pub Arc<dyn ObjectStore>);

impl ObjectStoreClient {
    /// Wrap a concrete [`ObjectStore`] implementation.
    pub fn new(store: impl ObjectStore) -> Self {
        Self(Arc::new(store))
    }

    /// Verify that the backing store is reachable.
    ///
    /// Issues a HEAD for a probe key — a not-found response is treated as
    /// success (the bucket/container exists), any other error is propagated.
    #[tracing::instrument(name = "object.verify", skip(self))]
    pub async fn verify_reachable(&self) -> Result<(), Error> {
        let path = Path::from("_nvisy_verify_probe");
        match self.0.head(&path).await {
            Ok(_) => Ok(()),
            Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(from_object_store(e)),
        }
    }

    /// List object keys under `prefix`.
    ///
    /// Returns all matching keys in a single `Vec`. For lazy iteration,
    /// use [`list_stream`](Self::list_stream) instead.
    #[tracing::instrument(name = "object.list", skip(self), fields(prefix))]
    pub async fn list(
        &self,
        prefix: &str,
    ) -> Result<Vec<ObjectMeta>, Error> {
        let prefix = if prefix.is_empty() {
            None
        } else {
            Some(Path::from(prefix))
        };
        self.0
            .list(prefix.as_ref())
            .try_collect()
            .await
            .map_err(from_object_store)
    }

    /// Lazily stream object metadata under `prefix`.
    #[tracing::instrument(name = "object.list_stream", skip(self), fields(prefix))]
    pub fn list_stream(
        &self,
        prefix: &str,
    ) -> BoxStream<'_, Result<ObjectMeta, Error>> {
        let prefix = if prefix.is_empty() {
            None
        } else {
            Some(Path::from(prefix))
        };
        Box::pin(self.0.list(prefix.as_ref()).map_err(from_object_store))
    }

    /// Retrieve the raw bytes, content-type, and metadata stored at `key`.
    #[tracing::instrument(name = "object.get", skip(self), fields(key))]
    pub async fn get(&self, key: &str) -> Result<GetResult, Error> {
        let path = Path::from(key);
        let result = self.0.get(&path).await.map_err(from_object_store)?;
        let meta = result.meta.clone();
        let content_type = result
            .attributes
            .get(&object_store::Attribute::ContentType)
            .map(|v| v.to_string());
        let data = result.bytes().await.map_err(from_object_store)?;
        Ok(GetResult {
            data,
            content_type,
            meta,
        })
    }

    /// Upload `data` to `key`, optionally setting the content-type.
    pub async fn put(
        &self,
        key: &str,
        data: Bytes,
        content_type: Option<&str>,
    ) -> Result<PutResult, Error> {
        self.put_opts(key, data, PutMode::Overwrite, content_type).await
    }

    /// Upload `data` to `key` with the specified [`PutMode`].
    #[tracing::instrument(name = "object.put_opts", skip(self, data), fields(key, size = data.len()))]
    pub async fn put_opts(
        &self,
        key: &str,
        data: Bytes,
        mode: PutMode,
        content_type: Option<&str>,
    ) -> Result<PutResult, Error> {
        let path = Path::from(key);
        let payload = PutPayload::from(data);
        let mut opts = PutOptions {
            mode,
            ..Default::default()
        };
        if let Some(ct) = content_type {
            opts.attributes.insert(
                object_store::Attribute::ContentType,
                ct.to_string().into(),
            );
        }
        let result = self
            .0
            .put_opts(&path, payload, opts)
            .await
            .map_err(from_object_store)?;
        Ok(result.into())
    }

    /// Get object metadata without downloading the body.
    #[tracing::instrument(name = "object.head", skip(self), fields(key))]
    pub async fn head(&self, key: &str) -> Result<ObjectMeta, Error> {
        let path = Path::from(key);
        self.0.head(&path).await.map_err(from_object_store)
    }

    /// Delete the object at `key`.
    #[tracing::instrument(name = "object.delete", skip(self), fields(key))]
    pub async fn delete(&self, key: &str) -> Result<(), Error> {
        let path = Path::from(key);
        self.0.delete(&path).await.map_err(from_object_store)
    }

    /// Copy an object from `src` to `dst` within the same store.
    #[tracing::instrument(name = "object.copy", skip(self), fields(src, dst))]
    pub async fn copy(&self, src: &str, dst: &str) -> Result<(), Error> {
        let from = Path::from(src);
        let to = Path::from(dst);
        self.0.copy(&from, &to).await.map_err(from_object_store)
    }
}

/// Convert an [`object_store::Error`] into a [`nvisy_core::Error`].
fn from_object_store(err: object_store::Error) -> Error {
    let retryable = !matches!(
        err,
        object_store::Error::NotFound { .. }
            | object_store::Error::PermissionDenied { .. }
            | object_store::Error::Unauthenticated { .. }
            | object_store::Error::AlreadyExists { .. }
            | object_store::Error::Precondition { .. }
    );
    Error::runtime(err.to_string(), "object-store", retryable)
        .with_source(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::memory::InMemory;

    fn test_client() -> ObjectStoreClient {
        ObjectStoreClient::new(InMemory::new())
    }

    #[tokio::test]
    async fn put_and_get() {
        let client = test_client();
        let data = Bytes::from("hello world");
        client
            .put("test.txt", data.clone(), Some("text/plain"))
            .await
            .unwrap();

        let result = client.get("test.txt").await.unwrap();
        assert_eq!(result.data, data);
        assert_eq!(result.content_type.as_deref(), Some("text/plain"));
    }

    #[tokio::test]
    async fn get_returns_meta() {
        let client = test_client();
        let data = Bytes::from("abc");
        client.put("meta.bin", data, None).await.unwrap();

        let result = client.get("meta.bin").await.unwrap();
        assert_eq!(result.meta.size as usize, 3);
        assert_eq!(result.meta.location, Path::from("meta.bin"));
    }

    #[tokio::test]
    async fn put_returns_result() {
        let client = test_client();
        let result = client
            .put("etag.bin", Bytes::from("x"), None)
            .await
            .unwrap();
        assert!(result.e_tag.is_some());
    }

    #[tokio::test]
    async fn head() {
        let client = test_client();
        client
            .put("head.bin", Bytes::from("data"), None)
            .await
            .unwrap();

        let meta = client.head("head.bin").await.unwrap();
        assert_eq!(meta.size, 4);
        assert_eq!(meta.location, Path::from("head.bin"));
    }

    #[tokio::test]
    async fn head_not_found() {
        let client = test_client();
        let err = client.head("missing").await.unwrap_err();
        assert!(!err.is_retryable());
    }

    #[tokio::test]
    async fn delete() {
        let client = test_client();
        client
            .put("del.bin", Bytes::from("x"), None)
            .await
            .unwrap();
        client.delete("del.bin").await.unwrap();

        assert!(client.get("del.bin").await.is_err());
    }

    #[tokio::test]
    async fn copy() {
        let client = test_client();
        let data = Bytes::from("copy me");
        client.put("src.bin", data.clone(), None).await.unwrap();
        client.copy("src.bin", "dst.bin").await.unwrap();

        let result = client.get("dst.bin").await.unwrap();
        assert_eq!(result.data, data);
    }

    #[tokio::test]
    async fn list() {
        let client = test_client();
        for i in 0..3 {
            client
                .put(
                    &format!("dir/file{i}.txt"),
                    Bytes::from(format!("{i}")),
                    None,
                )
                .await
                .unwrap();
        }

        let items = client.list("dir/").await.unwrap();
        assert_eq!(items.len(), 3);
    }

    #[tokio::test]
    async fn list_stream() {
        use futures::StreamExt;
        let client = test_client();
        for i in 0..3 {
            client
                .put(
                    &format!("stream/f{i}.bin"),
                    Bytes::from(format!("{i}")),
                    None,
                )
                .await
                .unwrap();
        }

        let items: Vec<_> = client
            .list_stream("stream/")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(items.len(), 3);
    }

    #[tokio::test]
    async fn put_create_only() {
        let client = test_client();
        client
            .put_opts(
                "unique.bin",
                Bytes::from("first"),
                PutMode::Create,
                None,
            )
            .await
            .unwrap();

        let err = client
            .put_opts(
                "unique.bin",
                Bytes::from("second"),
                PutMode::Create,
                None,
            )
            .await
            .unwrap_err();
        assert!(!err.is_retryable());
    }

    #[tokio::test]
    async fn verify_reachable() {
        let client = test_client();
        client.verify_reachable().await.unwrap();
    }
}
