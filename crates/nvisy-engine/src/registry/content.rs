//! [`ContentHandle`]: async handle to stored content data and metadata.

use std::fmt;

use bytes::Bytes;
use fjall::Keyspace;
use nvisy_core::content::{ContentData, ContentMetadata, ContentSource};
use nvisy_core::{Error, ErrorKind, Result};
use uuid::Uuid;

use super::store::composite_key;

const COMPONENT: &str = "registry::content";

/// Lightweight handle to a content entry stored in the registry.
///
/// Holds references to the fjall keyspaces so it can read content data
/// and metadata on demand. Cloning is cheap: fjall handles are
/// internally `Arc`-wrapped.
#[derive(Clone)]
pub struct ContentHandle {
    /// Actor identity that owns this content entry.
    actor_id: Uuid,
    /// Source identifier for the stored content.
    content_source: ContentSource,
    /// Keyspace storing raw content bytes.
    content_ks: Keyspace,
    /// Keyspace storing serialized content metadata.
    content_meta_ks: Keyspace,
}

impl fmt::Debug for ContentHandle {
    /// Formats the handle for debugging, omitting keyspace internals.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContentHandle")
            .field("actor_id", &self.actor_id)
            .field("content_source", &self.content_source)
            .finish_non_exhaustive()
    }
}

impl ContentHandle {
    /// Creates a new handle from pre-resolved keyspaces.
    ///
    /// This is `pub(crate)` because only [`Registry`](crate::registry::Registry)
    /// should construct handles after verifying the entry exists.
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

    /// Returns the actor ID that owns this content.
    #[must_use]
    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }

    /// Reads the content bytes from the store.
    ///
    /// The read is dispatched to a blocking thread via
    /// [`spawn_blocking`](tokio::task::spawn_blocking) to avoid
    /// blocking the async runtime on fjall I/O.
    #[tracing::instrument(
        target = COMPONENT,
        name = "content.read_data",
        skip(self),
        fields(actor_id = %self.actor_id, source_id = %self.content_source.as_uuid()),
    )]
    pub async fn content_data(&self) -> Result<ContentData> {
        let key = composite_key(self.actor_id, self.content_source.as_uuid());
        let source = self.content_source;
        let ks = self.content_ks.clone();

        tokio::task::spawn_blocking(move || -> Result<ContentData> {
            let value = ks.get(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to read content data")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;

            let guard = value.ok_or_else(|| {
                Error::new(
                    ErrorKind::NotFound,
                    format!("content data not found: {}", source.as_uuid()),
                )
                .with_component(COMPONENT)
            })?;

            Ok(ContentData::new(source, Bytes::copy_from_slice(&guard)))
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "blocking task panicked")
                .with_component(COMPONENT)
                .with_source(err)
        })?
    }

    /// Reads the content metadata from the store.
    ///
    /// Returns [`ContentMetadata::default()`] when the metadata key
    /// exists but has no value (e.g. content registered without metadata).
    #[tracing::instrument(
        target = COMPONENT,
        name = "content.read_metadata",
        skip(self),
        fields(actor_id = %self.actor_id, source_id = %self.content_source.as_uuid()),
    )]
    pub async fn metadata(&self) -> Result<ContentMetadata> {
        let key = composite_key(self.actor_id, self.content_source.as_uuid());
        let ks = self.content_meta_ks.clone();

        tokio::task::spawn_blocking(move || -> Result<ContentMetadata> {
            let value = ks.get(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to read content metadata")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;

            match value {
                Some(guard) => serde_json::from_slice(&guard).map_err(|err| {
                    Error::new(
                        ErrorKind::Serialization,
                        "failed to deserialize content metadata",
                    )
                    .with_component(COMPONENT)
                    .with_source(err)
                }),
                None => Ok(ContentMetadata::default()),
            }
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "blocking task panicked")
                .with_component(COMPONENT)
                .with_source(err)
        })?
    }
}
