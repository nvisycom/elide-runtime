//! Fjall extension traits, composite key type, and async helpers.
//!
//! Provides ergonomic wrappers around fjall's [`Keyspace`] and
//! [`Database`] with consistent error handling for the registry module.

use std::path::Path;

use fjall::{Database, Keyspace, KeyspaceCreateOptions, KvSeparationOptions};
use nvisy_core::{Error, ErrorKind, Result};
use uuid::Uuid;

const COMPONENT: &str = "registry";

/// 32-byte actor-scoped key: `[actor_id: 16][resource_id: 16]`.
///
/// Every registry entry is scoped to an actor. This type encodes
/// that invariant and provides ergonomic construction from two UUIDs.
#[derive(Clone, Copy)]
pub(crate) struct CompositeKey([u8; 32]);

impl CompositeKey {
    /// Build a key from an actor UUID and a resource UUID.
    pub fn new(actor_id: Uuid, resource_id: Uuid) -> Self {
        let mut key = [0u8; 32];
        key[..16].copy_from_slice(actor_id.as_bytes());
        key[16..].copy_from_slice(resource_id.as_bytes());
        Self(key)
    }

    /// The raw 32-byte slice.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Extract the resource UUID from the trailing 16 bytes.
    pub fn resource_id(&self) -> Uuid {
        Uuid::from_bytes(self.0[16..].try_into().unwrap())
    }
}

impl AsRef<[u8]> for CompositeKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Extension trait for fjall [`Keyspace`] with consistent error mapping.
pub(crate) trait FjallKeyspaceExt {
    fn put(&self, key: CompositeKey, value: &[u8]) -> Result<()>;
    fn get_bytes(&self, key: CompositeKey) -> Result<Option<Vec<u8>>>;
    fn delete(&self, key: CompositeKey) -> Result<()>;
    fn exists(&self, key: CompositeKey) -> Result<bool>;
}

impl FjallKeyspaceExt for Keyspace {
    fn put(&self, key: CompositeKey, value: &[u8]) -> Result<()> {
        self.insert(*key.as_bytes(), value)
            .map_err(|err| fjall_err("failed to write entry", err))
    }

    fn get_bytes(&self, key: CompositeKey) -> Result<Option<Vec<u8>>> {
        self.get(*key.as_bytes())
            .map(|opt| opt.map(|guard| guard.to_vec()))
            .map_err(|err| fjall_err("failed to read entry", err))
    }

    fn delete(&self, key: CompositeKey) -> Result<()> {
        self.remove(*key.as_bytes())
            .map_err(|err| fjall_err("failed to remove entry", err))
    }

    fn exists(&self, key: CompositeKey) -> Result<bool> {
        self.contains_key(*key.as_bytes())
            .map_err(|err| fjall_err("failed to check key", err))
    }
}

/// Extension trait for fjall [`Database`] with consistent error mapping.
pub(crate) trait FjallDatabaseExt {
    fn open_at(path: &Path) -> Result<Database>;
    fn open_keyspace(&self, name: &str) -> Result<Keyspace>;
    fn open_blob_keyspace(&self, name: &str) -> Result<Keyspace>;
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

fn fjall_err(msg: impl Into<String>, err: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::new(ErrorKind::Internal, msg)
        .with_component(COMPONENT)
        .with_source(err)
}

/// Build a not-found error for a registry resource.
pub(crate) fn not_found(kind: &str, actor_id: Uuid, resource_id: Uuid) -> Error {
    Error::new(
        ErrorKind::NotFound,
        format!("{kind} not found: actor_id={actor_id}, {kind}_id={resource_id}"),
    )
    .with_component(COMPONENT)
}

/// Collect all composite keys sharing the given actor prefix.
pub(crate) fn collect_prefix_keys(ks: &Keyspace, prefix: &[u8]) -> Result<Vec<CompositeKey>> {
    ks.prefix(prefix)
        .map(|guard| {
            let key = guard
                .key()
                .map_err(|err| fjall_err("failed to iterate keyspace", err))?;
            let bytes: [u8; 32] = key.as_ref().try_into().map_err(|_| {
                Error::new(ErrorKind::Internal, "unexpected key length").with_component(COMPONENT)
            })?;
            Ok(CompositeKey(bytes))
        })
        .collect()
}

/// Extract sorted resource UUIDs for an actor from a keyspace.
pub(crate) fn extract_resource_ids(ks: &Keyspace, prefix: &[u8]) -> Result<Vec<Uuid>> {
    let keys = collect_prefix_keys(ks, prefix)?;
    let mut ids: Vec<Uuid> = keys.iter().map(|k| k.resource_id()).collect();
    ids.sort();
    Ok(ids)
}

/// List resource IDs from a keyspace for the given actor.
pub(crate) async fn list_ids(ks: &Keyspace, actor_id: Uuid) -> Result<Vec<Uuid>> {
    let prefix = actor_id.as_bytes().to_vec();
    let ks = ks.clone();
    blocking(move || extract_resource_ids(&ks, &prefix)).await
}

/// Remove all entries in a keyspace for an actor. Returns count removed.
pub(crate) async fn remove_all(ks: &Keyspace, db: &Database, actor_id: Uuid) -> Result<usize> {
    let prefix = actor_id.as_bytes().to_vec();
    let ks = ks.clone();
    let db = db.clone();

    blocking(move || {
        let keys = collect_prefix_keys(&ks, &prefix)?;
        let count = keys.len();
        for key in &keys {
            ks.delete(*key)?;
        }
        if count > 0 {
            db.sync()?;
        }
        Ok(count)
    })
    .await
}
