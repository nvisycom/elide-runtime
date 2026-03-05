use std::path::{Path, PathBuf};

use fjall::{Database, Keyspace, KeyspaceCreateOptions, KvSeparationOptions};
use nvisy_core::io::Content;
use nvisy_core::path::ContentSource;
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::context::Context;
use uuid::Uuid;

use super::content::ContentHandle;
use super::context::ContextHandle;
use crate::id::{ActorId, ContentId, ContextId};

/// Builds a 32-byte composite key: `[actor: 16][resource_id: 16]`.
fn make_key(actor: ActorId, id: Uuid) -> [u8; 32] {
    let mut key = [0u8; 32];
    key[..16].copy_from_slice(actor.as_uuid().as_bytes());
    key[16..].copy_from_slice(id.as_bytes());
    key
}

/// Actor-scoped content and context store backed by fjall.
///
/// Stores content data, content metadata, and contexts in three keyspaces.
/// Every key is a 32-byte composite of `[actor_id][resource_id]`, so all
/// operations are inherently scoped to a single actor.
///
/// All handles are internally `Arc`-wrapped, making `Registry` cheap to
/// clone and safe to share across threads.
#[derive(Clone)]
pub struct Registry {
    base_dir: PathBuf,
    db: Database,
    content: Keyspace,
    content_meta: Keyspace,
    contexts: Keyspace,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("base_dir", &self.base_dir)
            .finish_non_exhaustive()
    }
}

impl Registry {
    /// Opens (or creates) the fjall database at `path`.
    ///
    /// Three keyspaces are created:
    /// - `"content"` with blob separation for efficient large-value storage
    /// - `"content_meta"` with default configuration
    /// - `"contexts"` with default configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the database or keyspaces cannot be opened.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let base_dir = path.into();

        let db = Database::builder(&base_dir).open().map_err(|err| {
            Error::new(
                ErrorKind::Internal,
                format!(
                    "Failed to open registry database (path: {})",
                    base_dir.display()
                ),
            )
            .with_source(err)
        })?;

        let content = db
            .keyspace("content", || {
                KeyspaceCreateOptions::default()
                    .with_kv_separation(Some(KvSeparationOptions::default()))
            })
            .map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to open content keyspace").with_source(err)
            })?;

        let content_meta = db
            .keyspace("content_meta", KeyspaceCreateOptions::default)
            .map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to open content_meta keyspace")
                    .with_source(err)
            })?;

        let contexts = db
            .keyspace("contexts", KeyspaceCreateOptions::default)
            .map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to open contexts keyspace").with_source(err)
            })?;

        Ok(Self {
            base_dir,
            db,
            content,
            content_meta,
            contexts,
        })
    }

    /// Registers content, writing its bytes and metadata to the store.
    ///
    /// Returns a [`ContentHandle`] for subsequent reads.
    pub async fn register_content(
        &self,
        actor: ActorId,
        content: Content,
    ) -> Result<ContentHandle> {
        let content_source = content.content_source();
        let key = make_key(actor, content_source.as_uuid());
        let data = content.as_bytes().to_vec();

        let (_, content_metadata) = content.into_parts();
        let meta_bytes =
            serde_json::to_vec(&content_metadata.unwrap_or_default()).map_err(|err| {
                Error::new(
                    ErrorKind::Serialization,
                    "Failed to serialize content metadata",
                )
                .with_source(err)
            })?;

        let content_ks = self.content.clone();
        let meta_ks = self.content_meta.clone();
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            content_ks.insert(key, &data).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to write content data").with_source(err)
            })?;
            meta_ks.insert(key, &meta_bytes).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to write content metadata").with_source(err)
            })?;
            db.persist(fjall::PersistMode::SyncAll).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to persist database").with_source(err)
            })?;
            Ok(())
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err)
        })??;

        Ok(ContentHandle::new(
            actor,
            content_source,
            self.content.clone(),
            self.content_meta.clone(),
        ))
    }

    /// Looks up previously registered content by actor and content ID.
    ///
    /// Returns [`ErrorKind::NotFound`] if no entry exists for the given key.
    pub async fn read_content(&self, actor: ActorId, id: ContentId) -> Result<ContentHandle> {
        let key = make_key(actor, id.as_uuid());
        let content_ks = self.content.clone();

        let exists = tokio::task::spawn_blocking(move || -> Result<bool> {
            content_ks.contains_key(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to check content key").with_source(err)
            })
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err)
        })??;

        if !exists {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Content not found (actor: {actor}, id: {id})"),
            ));
        }

        let source = ContentSource::from(id.as_uuid());
        Ok(ContentHandle::new(
            actor,
            source,
            self.content.clone(),
            self.content_meta.clone(),
        ))
    }

    /// Removes a single content entry by actor and content ID.
    ///
    /// Returns [`ErrorKind::NotFound`] if no entry exists for the given key.
    pub async fn unregister_content(&self, actor: ActorId, id: ContentId) -> Result<()> {
        let key = make_key(actor, id.as_uuid());
        let content_ks = self.content.clone();
        let meta_ks = self.content_meta.clone();
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let exists = content_ks.contains_key(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to check content key").with_source(err)
            })?;

            if !exists {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!("Content not found (actor: {actor}, id: {id})"),
                ));
            }

            content_ks.remove(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to remove content data").with_source(err)
            })?;
            meta_ks.remove(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to remove content metadata")
                    .with_source(err)
            })?;
            db.persist(fjall::PersistMode::SyncAll).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to persist database").with_source(err)
            })?;
            Ok(())
        })
        .await
        .map_err(|err| Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err))?
    }

    /// Removes all content entries for an actor.
    ///
    /// Returns the number of entries removed.
    pub async fn unregister_all_content(&self, actor: ActorId) -> Result<usize> {
        let prefix = actor.as_uuid().as_bytes().to_vec();
        let content_ks = self.content.clone();
        let meta_ks = self.content_meta.clone();
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || -> Result<usize> {
            let keys: Vec<Vec<u8>> = content_ks
                .prefix(&prefix)
                .map(|guard| {
                    let key = guard.key().map_err(|err| {
                        Error::new(ErrorKind::Internal, "Failed to iterate content keyspace")
                            .with_source(err)
                    })?;
                    Ok(key.to_vec())
                })
                .collect::<Result<Vec<_>>>()?;

            let count = keys.len();

            for key in &keys {
                content_ks.remove(key).map_err(|err| {
                    Error::new(ErrorKind::Internal, "Failed to remove content data")
                        .with_source(err)
                })?;
                meta_ks.remove(key).map_err(|err| {
                    Error::new(ErrorKind::Internal, "Failed to remove content metadata")
                        .with_source(err)
                })?;
            }

            if count > 0 {
                db.persist(fjall::PersistMode::SyncAll).map_err(|err| {
                    Error::new(ErrorKind::Internal, "Failed to persist database").with_source(err)
                })?;
            }

            Ok(count)
        })
        .await
        .map_err(|err| Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err))?
    }

    /// Lists all content IDs for an actor.
    pub async fn list_content(&self, actor: ActorId) -> Result<Vec<ContentId>> {
        let prefix = actor.as_uuid().as_bytes().to_vec();
        let content_ks = self.content.clone();

        tokio::task::spawn_blocking(move || -> Result<Vec<ContentId>> {
            let mut ids = Vec::new();
            for guard in content_ks.prefix(&prefix) {
                let key = guard.key().map_err(|err| {
                    Error::new(ErrorKind::Internal, "Failed to iterate content keyspace")
                        .with_source(err)
                })?;
                if key.len() == 32
                    && let Ok(bytes) = <[u8; 16]>::try_from(&key[16..])
                {
                    ids.push(ContentId::from(Uuid::from_bytes(bytes)));
                }
            }
            ids.sort();
            Ok(ids)
        })
        .await
        .map_err(|err| Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err))?
    }

    /// Registers a context, serializing it as JSON.
    ///
    /// Returns a [`ContextHandle`] for subsequent reads.
    pub async fn register_context(
        &self,
        actor: ActorId,
        context: Context,
    ) -> Result<ContextHandle> {
        let source = context.source;
        let key = make_key(actor, source.as_uuid());

        let json_bytes = serde_json::to_vec(&context).map_err(|err| {
            Error::new(ErrorKind::Serialization, "Failed to serialize context").with_source(err)
        })?;

        let ctx_ks = self.contexts.clone();
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            ctx_ks.insert(key, &json_bytes).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to write context").with_source(err)
            })?;
            db.persist(fjall::PersistMode::SyncAll).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to persist database").with_source(err)
            })?;
            Ok(())
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err)
        })??;

        Ok(ContextHandle::new(actor, source, self.contexts.clone()))
    }

    /// Looks up a previously registered context by actor and context ID.
    ///
    /// Returns [`ErrorKind::NotFound`] if no entry exists for the given key.
    pub async fn read_context(&self, actor: ActorId, id: ContextId) -> Result<ContextHandle> {
        let key = make_key(actor, id.as_uuid());
        let ctx_ks = self.contexts.clone();

        let exists = tokio::task::spawn_blocking(move || -> Result<bool> {
            ctx_ks.contains_key(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to check context key").with_source(err)
            })
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err)
        })??;

        if !exists {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Context not found (actor: {actor}, id: {id})"),
            ));
        }

        let source = ContentSource::from(id.as_uuid());
        Ok(ContextHandle::new(actor, source, self.contexts.clone()))
    }

    /// Removes a single context entry by actor and context ID.
    ///
    /// Returns [`ErrorKind::NotFound`] if no entry exists for the given key.
    pub async fn unregister_context(&self, actor: ActorId, id: ContextId) -> Result<()> {
        let key = make_key(actor, id.as_uuid());
        let ctx_ks = self.contexts.clone();
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let exists = ctx_ks.contains_key(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to check context key").with_source(err)
            })?;

            if !exists {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!("Context not found (actor: {actor}, id: {id})"),
                ));
            }

            ctx_ks.remove(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to remove context").with_source(err)
            })?;
            db.persist(fjall::PersistMode::SyncAll).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to persist database").with_source(err)
            })?;
            Ok(())
        })
        .await
        .map_err(|err| Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err))?
    }

    /// Removes all context entries for an actor.
    ///
    /// Returns the number of entries removed.
    pub async fn unregister_all_contexts(&self, actor: ActorId) -> Result<usize> {
        let prefix = actor.as_uuid().as_bytes().to_vec();
        let ctx_ks = self.contexts.clone();
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || -> Result<usize> {
            let keys: Vec<Vec<u8>> = ctx_ks
                .prefix(&prefix)
                .map(|guard| {
                    let key = guard.key().map_err(|err| {
                        Error::new(ErrorKind::Internal, "Failed to iterate contexts keyspace")
                            .with_source(err)
                    })?;
                    Ok(key.to_vec())
                })
                .collect::<Result<Vec<_>>>()?;

            let count = keys.len();

            for key in &keys {
                ctx_ks.remove(key).map_err(|err| {
                    Error::new(ErrorKind::Internal, "Failed to remove context").with_source(err)
                })?;
            }

            if count > 0 {
                db.persist(fjall::PersistMode::SyncAll).map_err(|err| {
                    Error::new(ErrorKind::Internal, "Failed to persist database").with_source(err)
                })?;
            }

            Ok(count)
        })
        .await
        .map_err(|err| Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err))?
    }

    /// Lists all context IDs for an actor.
    pub async fn list_contexts(&self, actor: ActorId) -> Result<Vec<ContextId>> {
        let prefix = actor.as_uuid().as_bytes().to_vec();
        let ctx_ks = self.contexts.clone();

        tokio::task::spawn_blocking(move || -> Result<Vec<ContextId>> {
            let mut ids = Vec::new();
            for guard in ctx_ks.prefix(&prefix) {
                let key = guard.key().map_err(|err| {
                    Error::new(ErrorKind::Internal, "Failed to iterate contexts keyspace")
                        .with_source(err)
                })?;
                if key.len() == 32
                    && let Ok(bytes) = <[u8; 16]>::try_from(&key[16..])
                {
                    ids.push(ContextId::from(Uuid::from_bytes(bytes)));
                }
            }
            ids.sort();
            Ok(ids)
        })
        .await
        .map_err(|err| Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err))?
    }

    /// Returns the base directory path (the database location).
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::io::{Content, ContentData};
    use nvisy_ontology::context::Context;

    use super::*;

    fn open_temp_registry() -> (tempfile::TempDir, Registry) {
        let temp = tempfile::TempDir::new().unwrap();
        let registry = Registry::open(temp.path().join("data")).unwrap();
        (temp, registry)
    }

    #[tokio::test]
    async fn register_and_read_content() {
        let (_temp, registry) = open_temp_registry();
        let actor = ActorId::new();
        let content = Content::new(ContentData::from("Hello, world!"));

        let handle = registry.register_content(actor, content).await.unwrap();
        let data = handle.content_data().await.unwrap();
        assert_eq!(data.as_str().unwrap(), "Hello, world!");
    }

    #[tokio::test]
    async fn content_scoped_by_actor() {
        let (_temp, registry) = open_temp_registry();
        let actor_a = ActorId::new();
        let actor_b = ActorId::new();

        let content = Content::new(ContentData::from("actor A only"));
        let handle = registry.register_content(actor_a, content).await.unwrap();
        let id = ContentId::from(handle.content_source().as_uuid());

        // Actor B cannot see actor A's content
        let err = registry.read_content(actor_b, id).await.unwrap_err();
        assert_eq!(err.kind, ErrorKind::NotFound);

        // Actor A can
        registry.read_content(actor_a, id).await.unwrap();
    }

    #[tokio::test]
    async fn list_content_per_actor() {
        let (_temp, registry) = open_temp_registry();
        let actor_a = ActorId::new();
        let actor_b = ActorId::new();

        registry
            .register_content(actor_a, Content::new(ContentData::from("a1")))
            .await
            .unwrap();
        registry
            .register_content(actor_a, Content::new(ContentData::from("a2")))
            .await
            .unwrap();
        registry
            .register_content(actor_b, Content::new(ContentData::from("b1")))
            .await
            .unwrap();

        assert_eq!(registry.list_content(actor_a).await.unwrap().len(), 2);
        assert_eq!(registry.list_content(actor_b).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn unregister_content() {
        let (_temp, registry) = open_temp_registry();
        let actor = ActorId::new();
        let content = Content::new(ContentData::from("delete me"));
        let id = ContentId::from(content.content_source().as_uuid());
        registry.register_content(actor, content).await.unwrap();

        registry.unregister_content(actor, id).await.unwrap();

        let err = registry.read_content(actor, id).await.unwrap_err();
        assert_eq!(err.kind, ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn unregister_all_content() {
        let (_temp, registry) = open_temp_registry();
        let actor = ActorId::new();

        registry
            .register_content(actor, Content::new(ContentData::from("first")))
            .await
            .unwrap();
        registry
            .register_content(actor, Content::new(ContentData::from("second")))
            .await
            .unwrap();

        let deleted = registry.unregister_all_content(actor).await.unwrap();
        assert_eq!(deleted, 2);
        assert!(registry.list_content(actor).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn register_and_read_context() {
        let (_temp, registry) = open_temp_registry();
        let actor = ActorId::new();
        let ctx = Context::new("test-context", vec![]);

        let handle = registry.register_context(actor, ctx.clone()).await.unwrap();
        let read_ctx = handle.context().await.unwrap();
        assert_eq!(read_ctx.name, "test-context");
    }

    #[tokio::test]
    async fn context_scoped_by_actor() {
        let (_temp, registry) = open_temp_registry();
        let actor_a = ActorId::new();
        let actor_b = ActorId::new();

        let ctx = Context::new("private", vec![]);
        let handle = registry.register_context(actor_a, ctx).await.unwrap();
        let id = ContextId::from(handle.source().as_uuid());

        let err = registry.read_context(actor_b, id).await.unwrap_err();
        assert_eq!(err.kind, ErrorKind::NotFound);

        registry.read_context(actor_a, id).await.unwrap();
    }

    #[tokio::test]
    async fn list_contexts_per_actor() {
        let (_temp, registry) = open_temp_registry();
        let actor = ActorId::new();

        registry
            .register_context(actor, Context::new("ctx-1", vec![]))
            .await
            .unwrap();
        registry
            .register_context(actor, Context::new("ctx-2", vec![]))
            .await
            .unwrap();

        assert_eq!(registry.list_contexts(actor).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn unregister_context() {
        let (_temp, registry) = open_temp_registry();
        let actor = ActorId::new();
        let ctx = Context::new("remove-me", vec![]);
        let id = ContextId::from(ctx.source.as_uuid());

        registry.register_context(actor, ctx).await.unwrap();
        registry.unregister_context(actor, id).await.unwrap();

        let err = registry.read_context(actor, id).await.unwrap_err();
        assert_eq!(err.kind, ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn unregister_all_contexts() {
        let (_temp, registry) = open_temp_registry();
        let actor = ActorId::new();

        registry
            .register_context(actor, Context::new("c1", vec![]))
            .await
            .unwrap();
        registry
            .register_context(actor, Context::new("c2", vec![]))
            .await
            .unwrap();

        let deleted = registry.unregister_all_contexts(actor).await.unwrap();
        assert_eq!(deleted, 2);
        assert!(registry.list_contexts(actor).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn data_persists_across_reopen() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("data");
        let actor = ActorId::new();

        let content = Content::new(ContentData::from("persistent"));
        let id = ContentId::from(content.content_source().as_uuid());

        {
            let registry = Registry::open(&path).unwrap();
            registry.register_content(actor, content).await.unwrap();
        }

        let registry = Registry::open(&path).unwrap();
        let handle = registry.read_content(actor, id).await.unwrap();
        let data = handle.content_data().await.unwrap();
        assert_eq!(data.as_str().unwrap(), "persistent");
    }

    #[tokio::test]
    async fn base_dir() {
        let temp = tempfile::TempDir::new().unwrap();
        let base = temp.path().join("data");
        let registry = Registry::open(&base).unwrap();
        assert_eq!(registry.base_dir(), base);
    }
}
