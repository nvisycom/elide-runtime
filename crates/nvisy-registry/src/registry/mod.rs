//! [`Registry`]: actor-scoped content and context store backed by fjall.

use std::path::{Path, PathBuf};

use fjall::{Database, Keyspace, KeyspaceCreateOptions, KvSeparationOptions};
use nvisy_core::content::{Content, ContentSource};
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::context::Context;
use uuid::Uuid;

use crate::handler::{ContentHandle, ContextHandle};

const TARGET: &str = "nvisy_registry";
const COMPONENT: &str = "nvisy-registry";

/// Builds a 32-byte composite key: `[actor_id: 16][resource_id: 16]`.
///
/// Used by both [`ContentHandle`] and [`ContextHandle`] to scope every
/// read/write to a specific actor.
pub(crate) fn composite_key(actor_id: Uuid, resource_id: Uuid) -> [u8; 32] {
    let mut key = [0u8; 32];
    key[..16].copy_from_slice(actor_id.as_bytes());
    key[16..].copy_from_slice(resource_id.as_bytes());
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
    /// Filesystem path where the fjall database is stored.
    base_dir: PathBuf,
    /// Underlying fjall database handle.
    db: Database,
    /// Keyspace for raw content bytes (blob-separated).
    content_ks: Keyspace,
    /// Keyspace for serialized content metadata.
    content_meta_ks: Keyspace,
    /// Keyspace for serialized detection contexts.
    contexts_ks: Keyspace,
}

impl std::fmt::Debug for Registry {
    /// Formats the registry for debugging, showing only the base directory.
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
    /// - `"content"`: blob separation for efficient large-value storage
    /// - `"content_meta"`: default configuration
    /// - `"contexts"`: default configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the database or keyspaces cannot be opened.
    #[tracing::instrument(target = TARGET, name = "registry.open", fields(path = %path.as_ref().display()))]
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let base_dir = path.as_ref().to_path_buf();

        let db = Database::builder(&base_dir).open().map_err(|err| {
            Error::new(
                ErrorKind::Internal,
                format!("failed to open database: {}", base_dir.display()),
            )
            .with_component(COMPONENT)
            .with_source(err)
        })?;

        let content_ks = db
            .keyspace("content", || {
                KeyspaceCreateOptions::default()
                    .with_kv_separation(Some(KvSeparationOptions::default()))
            })
            .map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to open content keyspace")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;

        let content_meta_ks = db
            .keyspace("content_meta", KeyspaceCreateOptions::default)
            .map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to open content_meta keyspace")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;

        let contexts_ks = db
            .keyspace("contexts", KeyspaceCreateOptions::default)
            .map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to open contexts keyspace")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;

        tracing::debug!(target: TARGET, "registry opened");

        Ok(Self {
            base_dir,
            db,
            content_ks,
            content_meta_ks,
            contexts_ks,
        })
    }

    // -- Content operations --------------------------------------------------

    /// Registers content, writing its bytes and metadata to the store.
    ///
    /// Returns a [`ContentHandle`] for subsequent reads.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or the underlying write fails.
    #[tracing::instrument(
        target = TARGET,
        name = "registry.register_content",
        skip(self, content),
        fields(actor_id = %actor_id),
    )]
    pub async fn register_content(
        &self,
        actor_id: Uuid,
        content: Content,
    ) -> Result<ContentHandle> {
        let content_source = content.content_source();
        let key = composite_key(actor_id, content_source.as_uuid());
        let data = content.as_bytes().to_vec();

        let (_, content_metadata) = content.into_parts();
        let meta_bytes =
            serde_json::to_vec(&content_metadata.unwrap_or_default()).map_err(|err| {
                Error::new(
                    ErrorKind::Serialization,
                    "failed to serialize content metadata",
                )
                .with_component(COMPONENT)
                .with_source(err)
            })?;

        let content_ks = self.content_ks.clone();
        let meta_ks = self.content_meta_ks.clone();
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            content_ks.insert(key, &data).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to write content data")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;
            meta_ks.insert(key, &meta_bytes).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to write content metadata")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;
            db.persist(fjall::PersistMode::SyncAll).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to persist database")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;
            Ok(())
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "blocking task panicked")
                .with_component(COMPONENT)
                .with_source(err)
        })??;

        tracing::trace!(
            target: TARGET,
            source_id = %content_source.as_uuid(),
            "content registered",
        );

        Ok(ContentHandle::new(
            actor_id,
            content_source,
            self.content_ks.clone(),
            self.content_meta_ks.clone(),
        ))
    }

    /// Looks up previously registered content by actor and content ID.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] if no entry exists for the given key.
    #[tracing::instrument(
        target = TARGET,
        name = "registry.read_content",
        skip(self),
        fields(actor_id = %actor_id, content_id = %content_id),
    )]
    pub async fn read_content(&self, actor_id: Uuid, content_id: Uuid) -> Result<ContentHandle> {
        let key = composite_key(actor_id, content_id);
        let ks = self.content_ks.clone();

        let exists = tokio::task::spawn_blocking(move || -> Result<bool> {
            ks.contains_key(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to check content key")
                    .with_component(COMPONENT)
                    .with_source(err)
            })
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "blocking task panicked")
                .with_component(COMPONENT)
                .with_source(err)
        })??;

        if !exists {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("content not found: actor_id={actor_id}, content_id={content_id}"),
            )
            .with_component(COMPONENT));
        }

        let source = ContentSource::from(content_id);
        Ok(ContentHandle::new(
            actor_id,
            source,
            self.content_ks.clone(),
            self.content_meta_ks.clone(),
        ))
    }

    /// Removes a single content entry (data + metadata) by actor and content ID.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] if no entry exists for the given key.
    #[tracing::instrument(
        target = TARGET,
        name = "registry.unregister_content",
        skip(self),
        fields(actor_id = %actor_id, content_id = %content_id),
    )]
    pub async fn unregister_content(&self, actor_id: Uuid, content_id: Uuid) -> Result<()> {
        let key = composite_key(actor_id, content_id);
        let content_ks = self.content_ks.clone();
        let meta_ks = self.content_meta_ks.clone();
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let exists = content_ks.contains_key(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to check content key")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;

            if !exists {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!("content not found: actor_id={actor_id}, content_id={content_id}"),
                )
                .with_component(COMPONENT));
            }

            content_ks.remove(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to remove content data")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;
            meta_ks.remove(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to remove content metadata")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;
            db.persist(fjall::PersistMode::SyncAll).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to persist database")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;
            Ok(())
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "blocking task panicked")
                .with_component(COMPONENT)
                .with_source(err)
        })?
    }

    /// Removes all content entries (data + metadata) for an actor.
    ///
    /// Returns the number of entries removed.
    #[tracing::instrument(
        target = TARGET,
        name = "registry.unregister_all_content",
        skip(self),
        fields(actor_id = %actor_id, removed),
    )]
    pub async fn unregister_all_content(&self, actor_id: Uuid) -> Result<usize> {
        let prefix = actor_id.as_bytes().to_vec();
        let content_ks = self.content_ks.clone();
        let meta_ks = self.content_meta_ks.clone();
        let db = self.db.clone();

        let count = tokio::task::spawn_blocking(move || -> Result<usize> {
            let keys = collect_prefix_keys(&content_ks, &prefix)?;
            let count = keys.len();

            for key in &keys {
                content_ks.remove(key).map_err(|err| {
                    Error::new(ErrorKind::Internal, "failed to remove content data")
                        .with_component(COMPONENT)
                        .with_source(err)
                })?;
                meta_ks.remove(key).map_err(|err| {
                    Error::new(ErrorKind::Internal, "failed to remove content metadata")
                        .with_component(COMPONENT)
                        .with_source(err)
                })?;
            }

            if count > 0 {
                db.persist(fjall::PersistMode::SyncAll).map_err(|err| {
                    Error::new(ErrorKind::Internal, "failed to persist database")
                        .with_component(COMPONENT)
                        .with_source(err)
                })?;
            }

            Ok(count)
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "blocking task panicked")
                .with_component(COMPONENT)
                .with_source(err)
        })??;

        tracing::Span::current().record("removed", count);
        Ok(count)
    }

    /// Lists all content IDs for an actor, sorted in ascending order.
    #[tracing::instrument(
        target = TARGET,
        name = "registry.list_content",
        skip(self),
        fields(actor_id = %actor_id),
    )]
    pub async fn list_content(&self, actor_id: Uuid) -> Result<Vec<Uuid>> {
        let prefix = actor_id.as_bytes().to_vec();
        let ks = self.content_ks.clone();

        tokio::task::spawn_blocking(move || extract_resource_ids(&ks, &prefix))
            .await
            .map_err(|err| {
                Error::new(ErrorKind::Internal, "blocking task panicked")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?
    }

    // -- Context operations --------------------------------------------------

    /// Registers a context, serializing it as JSON.
    ///
    /// Returns a [`ContextHandle`] for subsequent reads.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or the underlying write fails.
    #[tracing::instrument(
        target = TARGET,
        name = "registry.register_context",
        skip(self, context),
        fields(actor_id = %actor_id),
    )]
    pub async fn register_context(
        &self,
        actor_id: Uuid,
        context: Context,
    ) -> Result<ContextHandle> {
        let source = context.source;
        let key = composite_key(actor_id, source.as_uuid());

        let json_bytes = serde_json::to_vec(&context).map_err(|err| {
            Error::new(ErrorKind::Serialization, "failed to serialize context")
                .with_component(COMPONENT)
                .with_source(err)
        })?;

        let ks = self.contexts_ks.clone();
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            ks.insert(key, &json_bytes).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to write context")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;
            db.persist(fjall::PersistMode::SyncAll).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to persist database")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;
            Ok(())
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "blocking task panicked")
                .with_component(COMPONENT)
                .with_source(err)
        })??;

        tracing::trace!(
            target: TARGET,
            source_id = %source.as_uuid(),
            "context registered",
        );

        Ok(ContextHandle::new(
            actor_id,
            source,
            self.contexts_ks.clone(),
        ))
    }

    /// Looks up a previously registered context by actor and context ID.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] if no entry exists for the given key.
    #[tracing::instrument(
        target = TARGET,
        name = "registry.read_context",
        skip(self),
        fields(actor_id = %actor_id, context_id = %context_id),
    )]
    pub async fn read_context(&self, actor_id: Uuid, context_id: Uuid) -> Result<ContextHandle> {
        let key = composite_key(actor_id, context_id);
        let ks = self.contexts_ks.clone();

        let exists = tokio::task::spawn_blocking(move || -> Result<bool> {
            ks.contains_key(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to check context key")
                    .with_component(COMPONENT)
                    .with_source(err)
            })
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "blocking task panicked")
                .with_component(COMPONENT)
                .with_source(err)
        })??;

        if !exists {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("context not found: actor_id={actor_id}, context_id={context_id}"),
            )
            .with_component(COMPONENT));
        }

        let source = ContentSource::from(context_id);
        Ok(ContextHandle::new(
            actor_id,
            source,
            self.contexts_ks.clone(),
        ))
    }

    /// Removes a single context entry by actor and context ID.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::NotFound`] if no entry exists for the given key.
    #[tracing::instrument(
        target = TARGET,
        name = "registry.unregister_context",
        skip(self),
        fields(actor_id = %actor_id, context_id = %context_id),
    )]
    pub async fn unregister_context(&self, actor_id: Uuid, context_id: Uuid) -> Result<()> {
        let key = composite_key(actor_id, context_id);
        let ks = self.contexts_ks.clone();
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            let exists = ks.contains_key(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to check context key")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;

            if !exists {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!("context not found: actor_id={actor_id}, context_id={context_id}"),
                )
                .with_component(COMPONENT));
            }

            ks.remove(key).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to remove context")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;
            db.persist(fjall::PersistMode::SyncAll).map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to persist database")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;
            Ok(())
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "blocking task panicked")
                .with_component(COMPONENT)
                .with_source(err)
        })?
    }

    /// Removes all context entries for an actor.
    ///
    /// Returns the number of entries removed.
    #[tracing::instrument(
        target = TARGET,
        name = "registry.unregister_all_contexts",
        skip(self),
        fields(actor_id = %actor_id, removed),
    )]
    pub async fn unregister_all_contexts(&self, actor_id: Uuid) -> Result<usize> {
        let prefix = actor_id.as_bytes().to_vec();
        let ks = self.contexts_ks.clone();
        let db = self.db.clone();

        let count = tokio::task::spawn_blocking(move || -> Result<usize> {
            let keys = collect_prefix_keys(&ks, &prefix)?;
            let count = keys.len();

            for key in &keys {
                ks.remove(key).map_err(|err| {
                    Error::new(ErrorKind::Internal, "failed to remove context")
                        .with_component(COMPONENT)
                        .with_source(err)
                })?;
            }

            if count > 0 {
                db.persist(fjall::PersistMode::SyncAll).map_err(|err| {
                    Error::new(ErrorKind::Internal, "failed to persist database")
                        .with_component(COMPONENT)
                        .with_source(err)
                })?;
            }

            Ok(count)
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "blocking task panicked")
                .with_component(COMPONENT)
                .with_source(err)
        })??;

        tracing::Span::current().record("removed", count);
        Ok(count)
    }

    /// Lists all context IDs for an actor, sorted in ascending order.
    #[tracing::instrument(
        target = TARGET,
        name = "registry.list_contexts",
        skip(self),
        fields(actor_id = %actor_id),
    )]
    pub async fn list_contexts(&self, actor_id: Uuid) -> Result<Vec<Uuid>> {
        let prefix = actor_id.as_bytes().to_vec();
        let ks = self.contexts_ks.clone();

        tokio::task::spawn_blocking(move || extract_resource_ids(&ks, &prefix))
            .await
            .map_err(|err| {
                Error::new(ErrorKind::Internal, "blocking task panicked")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?
    }

    /// Returns the base directory path where the database is stored.
    #[must_use]
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

/// Collects all raw keys from a keyspace that share the given prefix.
fn collect_prefix_keys(ks: &Keyspace, prefix: &[u8]) -> Result<Vec<Vec<u8>>> {
    ks.prefix(prefix)
        .map(|guard| {
            let key = guard.key().map_err(|err| {
                Error::new(ErrorKind::Internal, "failed to iterate keyspace")
                    .with_component(COMPONENT)
                    .with_source(err)
            })?;
            Ok(key.to_vec())
        })
        .collect()
}

/// Extracts sorted resource UUIDs from the trailing 16 bytes of each
/// 32-byte composite key that shares the given prefix.
fn extract_resource_ids(ks: &Keyspace, prefix: &[u8]) -> Result<Vec<Uuid>> {
    let mut ids = Vec::new();
    for guard in ks.prefix(prefix) {
        let key = guard.key().map_err(|err| {
            Error::new(ErrorKind::Internal, "failed to iterate keyspace")
                .with_component(COMPONENT)
                .with_source(err)
        })?;
        if key.len() == 32
            && let Ok(bytes) = <[u8; 16]>::try_from(&key[16..])
        {
            ids.push(Uuid::from_bytes(bytes));
        }
    }
    ids.sort();
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use nvisy_core::content::{Content, ContentData};
    use nvisy_ontology::context::Context;

    use super::*;

    /// Opens a temporary registry backed by a fresh [`tempfile::TempDir`].
    fn open_temp_registry() -> (tempfile::TempDir, Registry) {
        let temp = tempfile::TempDir::new().unwrap();
        let registry = Registry::open(temp.path().join("data")).unwrap();
        (temp, registry)
    }

    #[tokio::test]
    async fn register_and_read_content() {
        let (_temp, registry) = open_temp_registry();
        let actor_id = Uuid::now_v7();
        let content = Content::new(ContentData::from("Hello, world!"));

        let handle = registry.register_content(actor_id, content).await.unwrap();
        let data = handle.content_data().await.unwrap();
        assert_eq!(data.as_str().unwrap(), "Hello, world!");
    }

    #[tokio::test]
    async fn content_scoped_by_actor() {
        let (_temp, registry) = open_temp_registry();
        let actor_a = Uuid::now_v7();
        let actor_b = Uuid::now_v7();

        let content = Content::new(ContentData::from("actor A only"));
        let handle = registry.register_content(actor_a, content).await.unwrap();
        let content_id = handle.content_source().as_uuid();

        // Actor B cannot see actor A's content.
        let err = registry
            .read_content(actor_b, content_id)
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::NotFound);

        // Actor A can.
        registry.read_content(actor_a, content_id).await.unwrap();
    }

    #[tokio::test]
    async fn list_content_per_actor() {
        let (_temp, registry) = open_temp_registry();
        let actor_a = Uuid::now_v7();
        let actor_b = Uuid::now_v7();

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
        let actor_id = Uuid::now_v7();
        let content = Content::new(ContentData::from("delete me"));
        let content_id = content.content_source().as_uuid();
        registry.register_content(actor_id, content).await.unwrap();

        registry
            .unregister_content(actor_id, content_id)
            .await
            .unwrap();

        let err = registry
            .read_content(actor_id, content_id)
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn unregister_all_content() {
        let (_temp, registry) = open_temp_registry();
        let actor_id = Uuid::now_v7();

        registry
            .register_content(actor_id, Content::new(ContentData::from("first")))
            .await
            .unwrap();
        registry
            .register_content(actor_id, Content::new(ContentData::from("second")))
            .await
            .unwrap();

        let deleted = registry.unregister_all_content(actor_id).await.unwrap();
        assert_eq!(deleted, 2);
        assert!(registry.list_content(actor_id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn register_and_read_context() {
        let (_temp, registry) = open_temp_registry();
        let actor_id = Uuid::now_v7();
        let ctx = Context::new("test-context", vec![]);

        let handle = registry
            .register_context(actor_id, ctx.clone())
            .await
            .unwrap();
        let read_ctx = handle.context().await.unwrap();
        assert_eq!(read_ctx.name, "test-context");
    }

    #[tokio::test]
    async fn context_scoped_by_actor() {
        let (_temp, registry) = open_temp_registry();
        let actor_a = Uuid::now_v7();
        let actor_b = Uuid::now_v7();

        let ctx = Context::new("private", vec![]);
        let handle = registry.register_context(actor_a, ctx).await.unwrap();
        let context_id = handle.source().as_uuid();

        let err = registry
            .read_context(actor_b, context_id)
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::NotFound);

        registry.read_context(actor_a, context_id).await.unwrap();
    }

    #[tokio::test]
    async fn list_contexts_per_actor() {
        let (_temp, registry) = open_temp_registry();
        let actor_id = Uuid::now_v7();

        registry
            .register_context(actor_id, Context::new("ctx-1", vec![]))
            .await
            .unwrap();
        registry
            .register_context(actor_id, Context::new("ctx-2", vec![]))
            .await
            .unwrap();

        assert_eq!(registry.list_contexts(actor_id).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn unregister_context() {
        let (_temp, registry) = open_temp_registry();
        let actor_id = Uuid::now_v7();
        let ctx = Context::new("remove-me", vec![]);
        let context_id = ctx.source.as_uuid();

        registry.register_context(actor_id, ctx).await.unwrap();
        registry
            .unregister_context(actor_id, context_id)
            .await
            .unwrap();

        let err = registry
            .read_context(actor_id, context_id)
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn unregister_all_contexts() {
        let (_temp, registry) = open_temp_registry();
        let actor_id = Uuid::now_v7();

        registry
            .register_context(actor_id, Context::new("c1", vec![]))
            .await
            .unwrap();
        registry
            .register_context(actor_id, Context::new("c2", vec![]))
            .await
            .unwrap();

        let deleted = registry.unregister_all_contexts(actor_id).await.unwrap();
        assert_eq!(deleted, 2);
        assert!(registry.list_contexts(actor_id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn data_persists_across_reopen() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("data");
        let actor_id = Uuid::now_v7();

        let content = Content::new(ContentData::from("persistent"));
        let content_id = content.content_source().as_uuid();

        {
            let registry = Registry::open(&path).unwrap();
            registry.register_content(actor_id, content).await.unwrap();
        }

        let registry = Registry::open(&path).unwrap();
        let handle = registry.read_content(actor_id, content_id).await.unwrap();
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
