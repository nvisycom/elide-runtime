//! [`ContentHandle`]: async handle to stored content data and record.

use std::fmt;

use bytes::Bytes;
use fjall::Keyspace;
use nvisy_codec::content::{
    Content, ContentData, ContentDescriptor, ContentDigest, ContentRecord, ContentSource,
};
use nvisy_core::Result;
use uuid::Uuid;

use super::composite_key::CompositeKey;
use super::fjall_ext::{FjallKeyspaceExt, blocking, not_found};

/// Lightweight handle to a content entry stored in the registry.
///
/// Holds references to the fjall keyspaces so it can read content
/// data and its persisted [`ContentRecord`] on demand.
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

    /// Reads the persisted [`ContentRecord`] (descriptor + digest).
    ///
    /// Returns a default record (empty descriptor, zero digest) when
    /// no record was stored — historically the metadata keyspace can
    /// hold entries written before the record format existed.
    pub async fn record(&self) -> Result<ContentRecord> {
        let key = CompositeKey::new(self.actor_id, self.content_source.as_uuid());
        let ks = self.content_meta_ks.clone();

        blocking(move || match ks.get_bytes(key)? {
            Some(bytes) => Ok(serde_json::from_slice(&bytes)?),
            None => Ok(ContentRecord {
                descriptor: ContentDescriptor::default(),
                digest: ContentDigest {
                    size: 0,
                    sha256: String::new(),
                    detected_content_type: None,
                },
            }),
        })
        .await
    }

    /// Reads bytes + descriptor, returning a [`Content`] suitable for
    /// re-importing through the codec pipeline. The digest half of
    /// the record is dropped — codec resolution only needs the
    /// caller-supplied bits (extension, MIME hint).
    pub async fn content(&self) -> Result<Content> {
        let (data, record) = tokio::try_join!(self.content_data(), self.record())?;
        Ok(Content::with_descriptor(data, record.descriptor))
    }
}
