//! Fjall extension traits and async helpers for the registry module.
//!
//! [`FjallDatabaseExt`] wraps raw fjall database operations with
//! consistent error mapping to [`Error`].
//!
//! [`Error`]: nvisy_core::Error

use std::error;
use std::path::Path;

use fjall::{Database, Keyspace, KeyspaceCreateOptions, KvSeparationOptions};
use nvisy_core::{Error, ErrorKind, Result};
use tokio::task;
use uuid::Uuid;

const COMPONENT: &str = "registry";

/// Extension trait for fjall [`Database`] with consistent error mapping.
///
/// Provides ergonomic database and keyspace creation methods that
/// map fjall errors to [`Error`].
///
/// [`Error`]: nvisy_core::Error
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
    task::spawn_blocking(f).await.map_err(|err| {
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
