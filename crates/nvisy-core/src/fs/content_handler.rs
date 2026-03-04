use bytes::Bytes;
use fjall::Keyspace;

use std::fmt;

use crate::error::{Error, ErrorKind, Result};
use crate::fs::ContentMetadata;
use crate::io::ContentData;
use crate::path::ContentSource;

/// Lightweight handle to a content entry stored in the registry.
///
/// Holds references to the fjall keyspaces so it can read content data
/// and metadata on demand. Cloning is cheap because fjall handles are
/// internally `Arc`-wrapped.
#[derive(Clone)]
pub struct ContentHandler {
    content_source: ContentSource,
    content: Keyspace,
    metadata: Keyspace,
}

impl fmt::Debug for ContentHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContentHandler")
            .field("content_source", &self.content_source)
            .finish_non_exhaustive()
    }
}

impl ContentHandler {
    pub(crate) fn new(
        content_source: ContentSource,
        content: Keyspace,
        metadata: Keyspace,
    ) -> Self {
        Self {
            content_source,
            content,
            metadata,
        }
    }

    /// Returns the content source identifier.
    pub fn content_source(&self) -> ContentSource {
        self.content_source
    }

    /// Reads the content bytes from the store.
    pub async fn content_data(&self) -> Result<ContentData> {
        let key = self.content_source.as_uuid().as_bytes().to_vec();
        let source = self.content_source;
        let content_ks = self.content.clone();

        tokio::task::spawn_blocking(move || {
            let value = content_ks.get(&key).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to read content data").with_source(err)
            })?;

            let guard = value.ok_or_else(|| {
                Error::new(
                    ErrorKind::NotFound,
                    format!("Content data not found (id: {})", source.as_uuid()),
                )
            })?;

            Ok(ContentData::new(source, Bytes::copy_from_slice(&guard)))
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err)
        })?
    }

    /// Reads the content metadata from the store.
    pub async fn metadata(&self) -> Result<ContentMetadata> {
        let key = self.content_source.as_uuid().as_bytes().to_vec();
        let metadata_ks = self.metadata.clone();

        tokio::task::spawn_blocking(move || {
            let value = metadata_ks.get(&key).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to read content metadata").with_source(err)
            })?;

            match value {
                Some(guard) => serde_json::from_slice(&guard).map_err(|err| {
                    Error::new(
                        ErrorKind::Serialization,
                        "Failed to deserialize content metadata",
                    )
                    .with_source(err)
                }),
                None => Ok(ContentMetadata::default()),
            }
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err)
        })?
    }
}

#[cfg(test)]
mod tests {
    use crate::fs::ContentRegistry;
    use crate::io::{Content, ContentData};

    #[tokio::test]
    async fn test_handler_has_valid_source() {
        let temp = tempfile::TempDir::new().unwrap();
        let registry = ContentRegistry::open(temp.path().join("content")).unwrap();
        let content = Content::new(ContentData::from("test data"));
        let handler = registry.register(content).await.unwrap();

        assert!(!handler.content_source().as_uuid().is_nil());
    }

    #[tokio::test]
    async fn test_content_data_reads_correctly() {
        let temp = tempfile::TempDir::new().unwrap();
        let registry = ContentRegistry::open(temp.path().join("content")).unwrap();
        let content = Content::new(ContentData::from("hello bytes"));
        let handler = registry.register(content).await.unwrap();

        let data = handler.content_data().await.unwrap();
        assert_eq!(data.as_str().unwrap(), "hello bytes");
        assert_eq!(data.content_source, handler.content_source());
    }

    #[tokio::test]
    async fn test_metadata_default_when_no_metadata() {
        let temp = tempfile::TempDir::new().unwrap();
        let registry = ContentRegistry::open(temp.path().join("content")).unwrap();
        let content = Content::new(ContentData::from("no metadata"));
        let handler = registry.register(content).await.unwrap();

        let metadata = handler.metadata().await.unwrap();
        assert!(!metadata.has_path());
    }

    #[tokio::test]
    async fn test_metadata_with_path() {
        use crate::fs::ContentMetadata;

        let temp = tempfile::TempDir::new().unwrap();
        let registry = ContentRegistry::open(temp.path().join("content")).unwrap();

        let data = ContentData::from("with metadata");
        let meta = ContentMetadata::with_path("document.pdf");
        let content = Content::with_metadata(data, meta);
        let handler = registry.register(content).await.unwrap();

        let metadata = handler.metadata().await.unwrap();
        assert!(metadata.has_path());
        assert_eq!(metadata.filename(), Some("document.pdf"));
    }

    #[tokio::test]
    async fn test_clone_shares_source() {
        let temp = tempfile::TempDir::new().unwrap();
        let registry = ContentRegistry::open(temp.path().join("content")).unwrap();
        let content = Content::new(ContentData::from("shared"));
        let handler1 = registry.register(content).await.unwrap();
        let handler2 = handler1.clone();

        assert_eq!(
            handler1.content_source().as_uuid(),
            handler2.content_source().as_uuid()
        );
    }
}
