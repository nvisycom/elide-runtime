use std::path::{Path, PathBuf};

use fjall::{Database, Keyspace, KeyspaceCreateOptions, KvSeparationOptions};
use uuid::Uuid;

use crate::error::{Error, ErrorKind, Result};
use crate::fs::ContentHandler;
use crate::io::Content;
use crate::path::ContentSource;

/// Content store backed by fjall, an embedded LSM key-value database.
///
/// Stores content data and metadata in two keyspaces:
/// - `content`: raw bytes with blob separation for large values
/// - `metadata`: JSON-serialized [`ContentMetadata`](crate::fs::ContentMetadata)
///
/// All handles are internally `Arc`-wrapped, making `ContentRegistry` cheap
/// to clone and safe to share across threads.
#[derive(Clone)]
pub struct ContentRegistry {
    base_dir: PathBuf,
    db: Database,
    content: Keyspace,
    metadata: Keyspace,
}

impl std::fmt::Debug for ContentRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentRegistry")
            .field("base_dir", &self.base_dir)
            .finish_non_exhaustive()
    }
}

impl ContentRegistry {
    /// Opens (or creates) the fjall database at `path`.
    ///
    /// Two keyspaces are created:
    /// - `"content"` with blob separation for efficient large-value storage
    /// - `"metadata"` with default configuration
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
                    "Failed to open content database (path: {})",
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
                Error::new(ErrorKind::Internal, "Failed to open content keyspace")
                    .with_source(err)
            })?;

        let metadata = db
            .keyspace("metadata", KeyspaceCreateOptions::default)
            .map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to open metadata keyspace")
                    .with_source(err)
            })?;

        Ok(Self {
            base_dir,
            db,
            content,
            metadata,
        })
    }

    /// Registers content, writing its bytes and metadata to the store.
    ///
    /// Returns a [`ContentHandler`] for subsequent reads.
    pub async fn register(&self, content: Content) -> Result<ContentHandler> {
        let content_source = content.content_source();
        let key = content_source.as_uuid().as_bytes().to_vec();
        let data = content.as_bytes().to_vec();

        let (_, content_metadata) = content.into_parts();
        let meta_bytes = serde_json::to_vec(&content_metadata.unwrap_or_default()).map_err(
            |err| {
                Error::new(ErrorKind::Serialization, "Failed to serialize content metadata")
                    .with_source(err)
            },
        )?;

        let content_ks = self.content.clone();
        let metadata_ks = self.metadata.clone();
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || {
            content_ks.insert(&key, &data).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to write content data").with_source(err)
            })?;
            metadata_ks.insert(&key, &meta_bytes).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to write content metadata")
                    .with_source(err)
            })?;
            db.persist(fjall::PersistMode::SyncAll).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to persist database").with_source(err)
            })?;
            Ok::<(), Error>(())
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err)
        })??;

        Ok(ContentHandler::new(
            content_source,
            self.content.clone(),
            self.metadata.clone(),
        ))
    }

    /// Looks up previously registered content by UUID.
    ///
    /// Returns [`ErrorKind::NotFound`] if no entry exists for the given id.
    pub async fn read(&self, id: Uuid) -> Result<ContentHandler> {
        let key = id.as_bytes().to_vec();
        let content_ks = self.content.clone();

        let exists = tokio::task::spawn_blocking(move || {
            content_ks.contains_key(&key).map_err(|err| {
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
                format!("Content not found (id: {id})"),
            ));
        }

        let source = ContentSource::from(id);
        Ok(ContentHandler::new(
            source,
            self.content.clone(),
            self.metadata.clone(),
        ))
    }

    /// Removes a single content entry by UUID.
    ///
    /// Returns [`ErrorKind::NotFound`] if no entry exists for the given id.
    pub async fn unregister(&self, id: Uuid) -> Result<()> {
        let key = id.as_bytes().to_vec();
        let content_ks = self.content.clone();
        let metadata_ks = self.metadata.clone();
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || {
            let exists = content_ks.contains_key(&key).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to check content key").with_source(err)
            })?;

            if !exists {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!("Content not found (id: {id})"),
                ));
            }

            content_ks.remove(&key).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to remove content data").with_source(err)
            })?;
            metadata_ks.remove(&key).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to remove content metadata")
                    .with_source(err)
            })?;
            db.persist(fjall::PersistMode::SyncAll).map_err(|err| {
                Error::new(ErrorKind::Internal, "Failed to persist database").with_source(err)
            })?;
            Ok(())
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err)
        })?
    }

    /// Removes all content entries.
    ///
    /// Returns the number of entries removed.
    pub async fn unregister_all(&self) -> Result<usize> {
        let content_ks = self.content.clone();
        let metadata_ks = self.metadata.clone();
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || {
            let keys: Vec<Vec<u8>> = content_ks
                .iter()
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
                metadata_ks.remove(key).map_err(|err| {
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
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err)
        })?
    }

    /// Lists all content UUIDs stored in the registry (sorted).
    pub async fn list(&self) -> Result<Vec<Uuid>> {
        let content_ks = self.content.clone();

        tokio::task::spawn_blocking(move || {
            let mut ids = Vec::new();
            for guard in content_ks.iter() {
                let key = guard.key().map_err(|err| {
                    Error::new(ErrorKind::Internal, "Failed to iterate content keyspace")
                        .with_source(err)
                })?;
                if let Ok(bytes) = <[u8; 16]>::try_from(&*key) {
                    ids.push(Uuid::from_bytes(bytes));
                }
            }
            ids.sort();
            Ok(ids)
        })
        .await
        .map_err(|err| {
            Error::new(ErrorKind::Internal, "Blocking task panicked").with_source(err)
        })?
    }

    /// Returns the base directory path (the database location).
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

#[cfg(test)]
mod tests {
    use crate::io::{Content, ContentData};

    use super::*;

    fn open_temp_registry() -> (tempfile::TempDir, ContentRegistry) {
        let temp = tempfile::TempDir::new().unwrap();
        let registry = ContentRegistry::open(temp.path().join("content")).unwrap();
        (temp, registry)
    }

    #[tokio::test]
    async fn test_register_and_read() {
        let (_temp, registry) = open_temp_registry();
        let content = Content::new(ContentData::from("Hello, world!"));
        let handler = registry.register(content).await.unwrap();

        let data = handler.content_data().await.unwrap();
        assert_eq!(data.as_str().unwrap(), "Hello, world!");
        assert_eq!(data.content_source, handler.content_source());
    }

    #[tokio::test]
    async fn test_base_dir() {
        let temp = tempfile::TempDir::new().unwrap();
        let base = temp.path().join("content");
        let registry = ContentRegistry::open(&base).unwrap();
        assert_eq!(registry.base_dir(), base);
    }

    #[tokio::test]
    async fn test_register_multiple() {
        let (_temp, registry) = open_temp_registry();

        let h1 = registry
            .register(Content::new(ContentData::from("first")))
            .await
            .unwrap();
        let h2 = registry
            .register(Content::new(ContentData::from("second")))
            .await
            .unwrap();

        assert_ne!(
            h1.content_source().as_uuid(),
            h2.content_source().as_uuid()
        );
    }

    #[tokio::test]
    async fn test_unregister() {
        let (_temp, registry) = open_temp_registry();
        let content = Content::new(ContentData::from("delete me"));
        let id = content.content_source().as_uuid();
        let _handler = registry.register(content).await.unwrap();

        registry.unregister(id).await.unwrap();

        let err = registry.read(id).await.unwrap_err();
        assert_eq!(err.kind, ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn test_unregister_not_found() {
        let (_temp, registry) = open_temp_registry();

        let id = Uuid::new_v4();
        let err = registry.unregister(id).await.unwrap_err();
        assert_eq!(err.kind, ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn test_read_via_registry() {
        let (_temp, registry) = open_temp_registry();
        let content = Content::new(ContentData::from("read me"));
        let id = content.content_source().as_uuid();
        registry.register(content).await.unwrap();

        let read_handler = registry.read(id).await.unwrap();
        let data = read_handler.content_data().await.unwrap();
        assert_eq!(data.as_str().unwrap(), "read me");
    }

    #[tokio::test]
    async fn test_read_not_found() {
        let (_temp, registry) = open_temp_registry();

        let id = Uuid::new_v4();
        let err = registry.read(id).await.unwrap_err();
        assert_eq!(err.kind, ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn test_list_empty() {
        let (_temp, registry) = open_temp_registry();

        let ids = registry.list().await.unwrap();
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn test_list() {
        let (_temp, registry) = open_temp_registry();

        let c1 = Content::new(ContentData::from("first"));
        let c2 = Content::new(ContentData::from("second"));
        let id1 = c1.content_source().as_uuid();
        let id2 = c2.content_source().as_uuid();

        registry.register(c1).await.unwrap();
        registry.register(c2).await.unwrap();

        let mut ids = registry.list().await.unwrap();
        ids.sort();
        let mut expected = vec![id1, id2];
        expected.sort();
        assert_eq!(ids, expected);
    }

    #[tokio::test]
    async fn test_unregister_all() {
        let (_temp, registry) = open_temp_registry();

        registry
            .register(Content::new(ContentData::from("first")))
            .await
            .unwrap();
        registry
            .register(Content::new(ContentData::from("second")))
            .await
            .unwrap();

        let deleted = registry.unregister_all().await.unwrap();
        assert_eq!(deleted, 2);

        let ids = registry.list().await.unwrap();
        assert!(ids.is_empty());
    }
}
