//! [`ContextHandle`]: async handle to a stored detection context.

use std::fmt;

use fjall::Keyspace;
use nvisy_core::content::ContentSource;
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::context::Context;
use uuid::Uuid;

use super::store::composite_key;

const TARGET: &str = "nvisy_engine::registry::context";

/// Lightweight handle to a context entry stored in the registry.
///
/// Holds a reference to the contexts keyspace so it can deserialize the
/// stored JSON on demand. Cloning is cheap: fjall handles are
/// internally `Arc`-wrapped.
#[derive(Clone)]
pub struct ContextHandle {
    /// Actor identity that owns this context entry.
    actor_id: Uuid,
    /// Content source this context is associated with.
    source: ContentSource,
    /// Keyspace storing serialized context JSON.
    contexts_ks: Keyspace,
}

impl fmt::Debug for ContextHandle {
    /// Formats the handle for debugging, omitting keyspace internals.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContextHandle")
            .field("actor_id", &self.actor_id)
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

impl ContextHandle {
    /// Creates a new handle from a pre-resolved keyspace.
    ///
    /// This is `pub(crate)` because only [`Registry`](crate::registry::Registry)
    /// should construct handles after verifying the entry exists.
    pub(crate) fn new(actor_id: Uuid, source: ContentSource, contexts_ks: Keyspace) -> Self {
        Self {
            actor_id,
            source,
            contexts_ks,
        }
    }

    /// Returns the content source identifier.
    #[must_use]
    pub fn source(&self) -> ContentSource {
        self.source
    }

    /// Returns the actor ID that owns this context.
    #[must_use]
    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }

    /// Reads and deserializes the context from the store.
    ///
    /// The read is dispatched to a blocking thread via
    /// [`spawn_blocking`](tokio::task::spawn_blocking) to avoid
    /// blocking the async runtime on fjall I/O.
    #[tracing::instrument(
        target = TARGET,
        name = "context.read",
        skip(self),
        fields(actor_id = %self.actor_id, source_id = %self.source.as_uuid()),
    )]
    pub async fn context(&self) -> Result<Context> {
        let key = composite_key(self.actor_id, self.source.as_uuid());
        let ks = self.contexts_ks.clone();

        tokio::task::spawn_blocking(move || -> Result<Context> {
            let value = ks.get(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to read context")
                    .with_component(TARGET)
                    .with_source(err)
            })?;

            let guard = value.ok_or_else(|| {
                Error::new(ErrorKind::NotFound, "context data not found").with_component(TARGET)
            })?;

            serde_json::from_slice(&guard).map_err(|err| {
                Error::new(ErrorKind::Serialization, "failed to deserialize context")
                    .with_component(TARGET)
                    .with_source(err)
            })
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "blocking task panicked")
                .with_component(TARGET)
                .with_source(err)
        })?
    }
}
