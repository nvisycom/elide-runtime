//! [`ContentHandle`]: async handle to stored content data and metadata.

use std::fmt;

use bytes::Bytes;
use fjall::Keyspace;
use nvisy_core::Result;
use nvisy_core::content::{Content, ContentData, ContentMetadata, ContentSource};
use uuid::Uuid;

use super::composite_key::CompositeKey;
use super::fjall_ext::{FjallKeyspaceExt, blocking, not_found};

/// Lightweight handle to a content entry stored in the registry.
///
/// Holds references to the fjall keyspaces so it can read content data
/// and metadata on demand.
#[derive(Clone)]
pub struct ContentHandle {
    actor_id: Uuid,
    content_source: ContentSource,
    content_ks: Keyspace,
    content_meta_ks: Keyspace,
}

impl fmt::Debug for ContentHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContentHandle")
            .field("actor_id", &self.actor_id)
            .field("content_source", &self.content_source)
            .finish_non_exhaustive()
    }
}

impl ContentHandle {
    pub(crate) fn new(
        actor_id: Uuid,
        content_source: ContentSource,
        content_ks: Keyspace,
        content_meta_ks: Keyspace,
    ) -> Self {
        Self {
            actor_id,
            content_source,
            content_ks,
            content_meta_ks,
        }
    }

    /// Returns the content source identifier.
    #[must_use]
    pub fn content_source(&self) -> ContentSource {
        self.content_source
    }

    /// Reads the content bytes from the store.
    pub async fn content_data(&self) -> Result<ContentData> {
        let key = CompositeKey::new(self.actor_id, self.content_source.as_uuid());
        let source = self.content_source;
        let ks = self.content_ks.clone();

        blocking(move || {
            let bytes = ks
                .get_bytes(key)?
                .ok_or_else(|| not_found("content", Uuid::nil(), source.as_uuid()))?;
            Ok(ContentData::new(source, Bytes::from(bytes)))
        })
        .await
    }

    /// Reads the content metadata from the store.
    pub async fn metadata(&self) -> Result<ContentMetadata> {
        let key = CompositeKey::new(self.actor_id, self.content_source.as_uuid());
        let ks = self.content_meta_ks.clone();

        blocking(move || match ks.get_bytes(key)? {
            Some(bytes) => Ok(serde_json::from_slice(&bytes)?),
            None => Ok(ContentMetadata::default()),
        })
        .await
    }

    /// Reads both content data and metadata, returning a full [`Content`].
    pub async fn content(&self) -> Result<Content> {
        let (data, metadata) = tokio::try_join!(self.content_data(), self.metadata())?;
        Ok(Content::with_metadata(data, metadata))
    }
}
