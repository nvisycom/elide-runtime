use std::fmt;

use fjall::Keyspace;
use nvisy_core::path::ContentSource;
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::context::Context;

use crate::id::ActorId;

/// Lightweight handle to a context entry stored in the registry.
///
/// Holds a reference to the contexts keyspace so it can deserialize the
/// stored JSON on demand. Cloning is cheap because fjall handles are
/// internally `Arc`-wrapped.
#[derive(Clone)]
pub struct ContextHandle {
    actor: ActorId,
    source: ContentSource,
    contexts: Keyspace,
}

impl fmt::Debug for ContextHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContextHandle")
            .field("actor", &self.actor)
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

impl ContextHandle {
    pub(crate) fn new(actor: ActorId, source: ContentSource, contexts: Keyspace) -> Self {
        Self {
            actor,
            source,
            contexts,
        }
    }

    /// Returns the content source identifier.
    pub fn source(&self) -> ContentSource {
        self.source
    }

    /// Returns the actor that owns this context.
    pub fn actor(&self) -> ActorId {
        self.actor
    }

    /// Reads and deserializes the context from the store.
    pub async fn context(&self) -> Result<Context> {
        let key = self.composite_key();
        let ctx_ks = self.contexts.clone();

        tokio::task::spawn_blocking(move || -> Result<Context> {
            let value = ctx_ks.get(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to read context").with_source(err)
            })?;

            let guard =
                value.ok_or_else(|| Error::new(ErrorKind::NotFound, "Context data not found"))?;

            serde_json::from_slice(&guard).map_err(|err| {
                Error::new(ErrorKind::Serialization, "Failed to deserialize context")
                    .with_source(err)
            })
        })
        .await
        .map_err(|err| Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err))?
    }

    fn composite_key(&self) -> [u8; 32] {
        let mut key = [0u8; 32];
        key[..16].copy_from_slice(self.actor.as_uuid().as_bytes());
        key[16..].copy_from_slice(self.source.as_uuid().as_bytes());
        key
    }
}
