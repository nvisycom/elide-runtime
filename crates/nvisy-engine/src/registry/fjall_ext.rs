//! Fjall extension traits and async helpers for the registry module.
//!
//! [`FjallKeyspaceExt`] and [`FjallDatabaseExt`] wrap raw fjall
//! operations with [`CompositeKey`] support and consistent error
//! mapping to [`nvisy_core::Error`].

use std::error;
use std::path::Path;

use fjall::{Database, Keyspace, KeyspaceCreateOptions, KvSeparationOptions};
use nvisy_core::{Error, ErrorKind, Result};
use uuid::Uuid;

use super::key::CompositeKey;

const COMPONENT: &str = "registry";

/// Extension trait for fjall [`Keyspace`] with consistent error mapping.
///
/// Wraps raw fjall operations to accept [`CompositeKey`] and return
/// [`nvisy_core::Result`] with standardized error context.
pub(crate) trait FjallKeyspaceExt {
    /// Insert a value at the given composite key.
    fn put(&self, key: CompositeKey, value: &[u8]) -> Result<()>;
    /// Read the value at a key, returning `None` if absent.
    fn get_bytes(&self, key: CompositeKey) -> Result<Option<Vec<u8>>>;
    /// Remove the entry at a key.
    fn delete(&self, key: CompositeKey) -> Result<()>;
    /// Check whether a key exists.
    fn exists(&self, key: CompositeKey) -> Result<bool>;
    /// List all resource UUIDs for the given actor (sorted).
    fn resource_ids(&self, actor_id: Uuid) -> Result<Vec<Uuid>>;
    /// Collect all composite keys sharing the given byte prefix.
    fn prefix_keys(&self, prefix: &[u8]) -> Result<Vec<CompositeKey>>;
}

impl FjallKeyspaceExt for Keyspace {
    fn put(&self, key: CompositeKey, value: &[u8]) -> Result<()> {
        self.insert(*key, value)
            .map_err(|err| fjall_err("failed to write entry", err))
    }

    fn get_bytes(&self, key: CompositeKey) -> Result<Option<Vec<u8>>> {
        self.get(*key)
            .map(|opt| opt.map(|guard| guard.to_vec()))
            .map_err(|err| fjall_err("failed to read entry", err))
    }

    fn delete(&self, key: CompositeKey) -> Result<()> {
        self.remove(*key)
            .map_err(|err| fjall_err("failed to remove entry", err))
    }

    fn exists(&self, key: CompositeKey) -> Result<bool> {
        self.contains_key(*key)
            .map_err(|err| fjall_err("failed to check key", err))
    }

    fn resource_ids(&self, actor_id: Uuid) -> Result<Vec<Uuid>> {
        let keys = self.prefix_keys(actor_id.as_bytes())?;
        let mut ids: Vec<Uuid> = keys.iter().map(|k| k.resource_id()).collect();
        ids.sort();
        Ok(ids)
    }

    fn prefix_keys(&self, prefix: &[u8]) -> Result<Vec<CompositeKey>> {
        self.prefix(prefix)
            .map(|guard| {
                let key = guard
                    .key()
                    .map_err(|err| fjall_err("failed to iterate keyspace", err))?;
                let bytes: [u8; 32] = key.as_ref().try_into().map_err(|_| {
                    Error::new(ErrorKind::Internal, "unexpected key length")
                        .with_component(COMPONENT)
                })?;
                Ok(CompositeKey::from(bytes))
            })
            .collect()
    }
}

/// Extension trait for fjall [`Database`] with consistent error mapping.
///
/// Provides ergonomic database and keyspace creation methods that
/// map fjall errors to [`nvisy_core::Error`].
pub(crate) trait FjallDatabaseExt {
    /// Open (or create) a database at the given path.
    fn open_at(path: &Path) -> Result<Database>;
    /// Open (or create) a keyspace with default options.
    fn open_keyspace(&self, name: &str) -> Result<Keyspace>;
    /// Open (or create) a keyspace with blob-separated storage
    /// (optimized for large values).
    fn open_blob_keyspace(&self, name: &str) -> Result<Keyspace>;
    /// Flush all pending writes to disk.
    fn sync(&self) -> Result<()>;
}

impl FjallDatabaseExt for Database {
    fn open_at(path: &Path) -> Result<Database> {
        Database::builder(path)
            .open()
            .map_err(|err| fjall_err(format!("failed to open database: {}", path.display()), err))
    }

    fn open_keyspace(&self, name: &str) -> Result<Keyspace> {
        self.keyspace(name, KeyspaceCreateOptions::default)
            .map_err(|err| fjall_err(format!("failed to open {name} keyspace"), err))
    }

    fn open_blob_keyspace(&self, name: &str) -> Result<Keyspace> {
        self.keyspace(name, || {
            KeyspaceCreateOptions::default()
                .with_kv_separation(Some(KvSeparationOptions::default()))
        })
        .map_err(|err| fjall_err(format!("failed to open {name} keyspace"), err))
    }

    fn sync(&self) -> Result<()> {
        self.persist(fjall::PersistMode::SyncAll)
            .map_err(|err| fjall_err("failed to persist database", err))
    }
}

/// Run a blocking closure on the tokio blocking thread pool.
///
/// All fjall I/O goes through this to avoid blocking the async runtime.
pub(crate) async fn blocking<F, T>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f).await.map_err(|err| {
        Error::new(ErrorKind::Internal, "blocking task panicked")
            .with_component(COMPONENT)
            .with_source(err)
    })?
}

/// Build a not-found error for a registry resource.
pub(crate) fn not_found(kind: &str, actor_id: Uuid, resource_id: Uuid) -> Error {
    Error::new(
        ErrorKind::NotFound,
        format!("{kind} not found: actor_id={actor_id}, {kind}_id={resource_id}"),
    )
    .with_component(COMPONENT)
}

fn fjall_err(msg: impl Into<String>, err: impl error::Error + Send + Sync + 'static) -> Error {
    Error::new(ErrorKind::Internal, msg)
        .with_component(COMPONENT)
        .with_source(err)
}
