//! File resource API over the engine's fjall registry.
//!
//! Surfaced as the [`FileRegistry`] extension trait on
//! [`RegistryHandle`] so file calls flow through the same handle
//! every other resource API takes —
//! `handle.put_file(actor, descriptor, bytes)`, etc.
//!
//! A file is a `(metadata, bytes)` pair: the bytes live in a
//! blob-separated `files_content` keyspace; the metadata
//! ([`FileMetadata`]) lives in a small JSON `files_metadata`
//! keyspace. The split lets [`FileRegistry::list_files`] enumerate
//! every file for an actor without paying the cost of loading the
//! bytes.
//!
//! Writes are content-first, metadata-second. A failure between
//! the two leaves orphan bytes (garbage-collectable later by a
//! sweep over `files_content` keys without a matching metadata
//! entry); the opposite order would leave a metadata row that
//! lists a file whose bytes can't be downloaded — strictly worse.
//!
//! Surface:
//!
//! - [`put_file`](FileRegistry::put_file) — write bytes +
//!   metadata; mints a UUIDv7, computes the SHA-256 digest.
//! - [`get_file`](FileRegistry::get_file) — read the metadata
//!   blob.
//! - [`get_file_bytes`](FileRegistry::get_file_bytes) — read the
//!   raw bytes.
//! - [`list_files`](FileRegistry::list_files) — list every
//!   metadata blob for an actor.
//! - [`delete_file`](FileRegistry::delete_file) — remove one
//!   file (bytes + metadata).
//! - [`delete_all_files`](FileRegistry::delete_all_files) — wipe
//!   every file for an actor.

use std::collections::HashMap;
use std::error::Error as StdError;

use bytes::Bytes;
use hipstr::HipStr;
use jiff::Timestamp;
use nvisy_core::{Error, Result};
use nvisy_schema::file::{FileLineage, FileMetadata};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::registry::{CompositeKey, RegistryHandle, blocking, not_found};

const COMPONENT: &str = "files";
const KIND: &str = "file";

/// Caller-supplied descriptor for a new upload. The engine
/// derives [`FileMetadata::id`], [`uploaded_at`], [`size`], and
/// [`digest`] at write time.
///
/// [`uploaded_at`]: FileMetadata::uploaded_at
/// [`size`]: FileMetadata::size
/// [`digest`]: FileMetadata::digest
#[derive(Debug, Clone, Default)]
pub struct FileDescriptor {
    /// Original filename, if any (e.g. parsed from
    /// `Content-Disposition`).
    pub filename: Option<HipStr<'static>>,
    /// Caller-supplied MIME hint.
    pub content_type: Option<HipStr<'static>>,
    /// File extension the codec resolves on (case-insensitive,
    /// no leading dot).
    pub extension: HipStr<'static>,
    /// Where the file came from. Uploads pass `None`; engine
    /// callers (today: the redaction apply path) stamp a
    /// [`FileLineage::RedactedFrom`] so the output is traceable
    /// back to its source.
    pub lineage: Option<FileLineage>,
    /// Doc-level labels (gate `DocumentPredicate::HasLabel`
    /// policies when a run later references this file).
    pub descriptor_labels: Vec<String>,
    /// Doc-level metadata (gate `DocumentPredicate::HasMetadata`
    /// policies).
    pub descriptor_metadata: HashMap<String, String>,
}

/// Extension trait adding file-resource CRUD to
/// [`RegistryHandle`].
///
/// Implemented for `RegistryHandle` itself; bring the trait into
/// scope (`use nvisy_engine::FileRegistry;`) to call its
/// methods.
pub trait FileRegistry {
    /// Write bytes + metadata for a new file. Engine mints a
    /// UUIDv7 id, computes the SHA-256 digest, stamps
    /// `uploaded_at`, and returns the assembled
    /// [`FileMetadata`]. Bytes are persisted before metadata so
    /// a partial failure leaves recoverable orphan bytes rather
    /// than a metadata row pointing at missing content.
    fn put_file(
        &self,
        actor_id: Uuid,
        descriptor: FileDescriptor,
        bytes: Bytes,
    ) -> impl Future<Output = Result<FileMetadata>> + Send;

    /// Read one file's metadata. Returns
    /// [`ErrorKind::NotFound`] when no entry exists at
    /// `(actor_id, file_id)`.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    fn get_file(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
    ) -> impl Future<Output = Result<FileMetadata>> + Send;

    /// Read one file's raw bytes. Returns
    /// [`ErrorKind::NotFound`] when no entry exists at
    /// `(actor_id, file_id)`.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    fn get_file_bytes(
        &self,
        actor_id: Uuid,
        file_id: Uuid,
    ) -> impl Future<Output = Result<Bytes>> + Send;

    /// List every file metadata blob for `actor_id`. Bytes are
    /// not loaded.
    fn list_files(&self, actor_id: Uuid) -> impl Future<Output = Result<Vec<FileMetadata>>> + Send;

    /// Remove one file (bytes + metadata). Returns
    /// [`ErrorKind::NotFound`] if the metadata entry was already
    /// absent. Best-effort on the bytes — silently skips if the
    /// bytes were already gone.
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    fn delete_file(&self, actor_id: Uuid, file_id: Uuid)
    -> impl Future<Output = Result<()>> + Send;

    /// Remove every file (bytes + metadata) belonging to
    /// `actor_id`. Returns the total count removed across both
    /// keyspaces.
    fn delete_all_files(&self, actor_id: Uuid) -> impl Future<Output = Result<usize>> + Send;
}

impl FileRegistry for RegistryHandle {
    async fn put_file(
        &self,
        actor_id: Uuid,
        descriptor: FileDescriptor,
        bytes: Bytes,
    ) -> Result<FileMetadata> {
        let file_id = Uuid::now_v7();
        let size = bytes.len() as u64;
        let digest = hex_sha256(&bytes);
        let metadata = FileMetadata {
            id: file_id,
            filename: descriptor.filename,
            content_type: descriptor.content_type,
            extension: descriptor.extension,
            size,
            digest,
            uploaded_at: Timestamp::now(),
            lineage: descriptor.lineage,
            descriptor_labels: descriptor.descriptor_labels,
            descriptor_metadata: descriptor.descriptor_metadata,
        };

        // Content-first, metadata-second. A partial failure here
        // leaves orphan bytes that a future sweep can reap; the
        // opposite order would advertise a file we can't serve.
        let key = CompositeKey::new(actor_id, file_id);
        let content_ks = self.files_content().clone();
        let metadata_ks = self.files_metadata().clone();
        let metadata_value = serde_json::to_vec(&metadata)?;
        let raw_bytes = bytes.to_vec();
        blocking(move || {
            content_ks.insert(*key, raw_bytes).map_err(fjall_err)?;
            metadata_ks
                .insert(*key, metadata_value)
                .map_err(fjall_err)?;
            Ok(())
        })
        .await?;

        Ok(metadata)
    }

    async fn get_file(&self, actor_id: Uuid, file_id: Uuid) -> Result<FileMetadata> {
        let key = CompositeKey::new(actor_id, file_id);
        let metadata_ks = self.files_metadata().clone();
        blocking(move || {
            let value = metadata_ks
                .get(*key)
                .map_err(fjall_err)?
                .ok_or_else(|| not_found(KIND, actor_id, file_id))?;
            serde_json::from_slice(&value).map_err(Into::into)
        })
        .await
    }

    async fn get_file_bytes(&self, actor_id: Uuid, file_id: Uuid) -> Result<Bytes> {
        let key = CompositeKey::new(actor_id, file_id);
        let content_ks = self.files_content().clone();
        blocking(move || {
            let value = content_ks
                .get(*key)
                .map_err(fjall_err)?
                .ok_or_else(|| not_found(KIND, actor_id, file_id))?;
            Ok(Bytes::from(value.to_vec()))
        })
        .await
    }

    async fn list_files(&self, actor_id: Uuid) -> Result<Vec<FileMetadata>> {
        let metadata_ks = self.files_metadata().clone();
        blocking(move || {
            let prefix = CompositeKey::actor_prefix(actor_id);
            let mut out: Vec<FileMetadata> = Vec::new();
            for guard in metadata_ks.prefix(prefix) {
                let (_, value) = guard.into_inner().map_err(fjall_err)?;
                let metadata: FileMetadata = serde_json::from_slice(&value)?;
                out.push(metadata);
            }
            Ok(out)
        })
        .await
    }

    async fn delete_file(&self, actor_id: Uuid, file_id: Uuid) -> Result<()> {
        let key = CompositeKey::new(actor_id, file_id);
        let metadata_ks = self.files_metadata().clone();
        let content_ks = self.files_content().clone();
        blocking(move || {
            if !metadata_ks.contains_key(*key).map_err(fjall_err)? {
                return Err(not_found(KIND, actor_id, file_id));
            }
            // Metadata-first, content-second on delete: removing
            // metadata makes the file invisible to subsequent
            // `list_files` / `get_file` calls; if the content
            // remove fails the file is logically gone, the bytes
            // are reapable later.
            metadata_ks.remove(*key).map_err(fjall_err)?;
            content_ks.remove(*key).map_err(fjall_err)?;
            Ok(())
        })
        .await
    }

    async fn delete_all_files(&self, actor_id: Uuid) -> Result<usize> {
        let metadata_ks = self.files_metadata().clone();
        let content_ks = self.files_content().clone();
        blocking(move || {
            let prefix = CompositeKey::actor_prefix(actor_id);
            let mut removed = 0usize;
            for keyspace in [&metadata_ks, &content_ks] {
                let keys: Vec<Vec<u8>> = keyspace
                    .prefix(prefix)
                    .map(|guard| {
                        let key = guard.key().map_err(fjall_err)?;
                        Ok(key.as_ref().to_vec())
                    })
                    .collect::<Result<Vec<_>>>()?;
                removed += keys.len();
                for key in keys {
                    keyspace.remove(&key).map_err(fjall_err)?;
                }
            }
            Ok(removed)
        })
        .await
    }
}

fn hex_sha256(bytes: &[u8]) -> HipStr<'static> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    HipStr::from(out)
}

fn fjall_err(err: impl StdError + Send + Sync + 'static) -> Error {
    Error::internal("fjall operation failed", COMPONENT).with_source(err)
}
