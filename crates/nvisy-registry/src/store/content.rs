use std::fmt;

use bytes::Bytes;
use fjall::Keyspace;
use nvisy_core::fs::ContentMetadata;
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;
use nvisy_core::{Error, ErrorKind, Result};
use uuid::Uuid;

/// Lightweight handle to a content entry stored in the registry.
///
/// Holds references to the fjall keyspaces so it can read content data
/// and metadata on demand. Cloning is cheap because fjall handles are
/// internally `Arc`-wrapped.
#[derive(Clone)]
pub struct ContentHandle {
    actor: Uuid,
    content_source: ContentSource,
    content: Keyspace,
    content_meta: Keyspace,
}

impl fmt::Debug for ContentHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContentHandle")
            .field("actor", &self.actor)
            .field("content_source", &self.content_source)
            .finish_non_exhaustive()
    }
}

impl ContentHandle {
    pub(crate) fn new(
        actor: Uuid,
        content_source: ContentSource,
        content: Keyspace,
        content_meta: Keyspace,
    ) -> Self {
        Self {
            actor,
            content_source,
            content,
            content_meta,
        }
    }

    /// Returns the content source identifier.
    pub fn content_source(&self) -> ContentSource {
        self.content_source
    }

    /// Returns the actor that owns this content.
    pub fn actor(&self) -> Uuid {
        self.actor
    }

    /// Reads the content bytes from the store.
    pub async fn content_data(&self) -> Result<ContentData> {
        let key = self.composite_key();
        let source = self.content_source;
        let content_ks = self.content.clone();

        tokio::task::spawn_blocking(move || -> Result<ContentData> {
            let value = content_ks.get(key).map_err(|err| {
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
        .map_err(|err| Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err))?
    }

    /// Reads the content metadata from the store.
    pub async fn metadata(&self) -> Result<ContentMetadata> {
        let key = self.composite_key();
        let meta_ks = self.content_meta.clone();

        tokio::task::spawn_blocking(move || -> Result<ContentMetadata> {
            let value = meta_ks.get(key).map_err(|err| {
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
        .map_err(|err| Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err))?
    }

    fn composite_key(&self) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[..16].copy_from_slice(self.actor.as_bytes());
        key[16..].copy_from_slice(self.content_source.as_uuid().as_bytes());
        key
    }
}
