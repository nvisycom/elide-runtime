//! [`RegistryHandle`]: owns the fjall [`Database`] + every
//! keyspace the engine resource modules read and write.
//!
//! Constructed once at server startup with
//! [`RegistryHandle::open`]; cheaply cloneable (the fjall types
//! are `Arc`-backed) so it can be passed to every resource API as
//! a borrowed handle.

use std::path::Path;
use std::sync::Arc;

use fjall::{Database, Keyspace};
use nvisy_core::Result;

use super::fjall_ext::FjallDatabaseExt;

/// Keyspace name constants. Each is one logical resource class.
const KS_POLICIES: &str = "policies";
const KS_CONTEXTS: &str = "contexts";
const KS_FILES_METADATA: &str = "files_metadata";
const KS_FILES_CONTENT: &str = "files_content";
const KS_RUN_HEADERS: &str = "run_headers";
const KS_RUN_DOCS: &str = "run_docs";

/// Cheaply-cloneable handle over the engine's fjall keyspaces.
#[derive(Clone)]
pub struct RegistryHandle {
    inner: Arc<RegistryInner>,
}

struct RegistryInner {
    db: Database,
    policies: Keyspace,
    contexts: Keyspace,
    files_metadata: Keyspace,
    files_content: Keyspace,
    run_headers: Keyspace,
    run_docs: Keyspace,
}

impl RegistryHandle {
    /// Open (or create) the engine database at `path` and pre-open
    /// every keyspace. Idempotent — re-opening an existing database
    /// at the same path returns a handle to the same data.
    pub fn open(path: &Path) -> Result<Self> {
        let db = Database::open_at(path)?;
        let policies = db.open_blob_keyspace(KS_POLICIES)?;
        let contexts = db.open_blob_keyspace(KS_CONTEXTS)?;
        let files_metadata = db.open_keyspace(KS_FILES_METADATA)?;
        let files_content = db.open_blob_keyspace(KS_FILES_CONTENT)?;
        let run_headers = db.open_keyspace(KS_RUN_HEADERS)?;
        let run_docs = db.open_blob_keyspace(KS_RUN_DOCS)?;
        Ok(Self {
            inner: Arc::new(RegistryInner {
                db,
                policies,
                contexts,
                files_metadata,
                files_content,
                run_headers,
                run_docs,
            }),
        })
    }

    /// Keyspace for persisted `Policy` resources. Blob-separated:
    /// policy bodies range from small to large (rule lists +
    /// catalogs).
    pub(crate) fn policies(&self) -> &Keyspace {
        &self.inner.policies
    }

    /// Keyspace for persisted `Context` resources. Blob-separated.
    pub(crate) fn contexts(&self) -> &Keyspace {
        &self.inner.contexts
    }

    /// Keyspace for [`FileMetadata`] blobs — small JSON
    /// descriptors keyed by `(actor_id, file_id)`. Plain
    /// keyspace (no blob separation) because the bodies are
    /// tiny; range-scan for `list_files` is cheap.
    ///
    /// [`FileMetadata`]: nvisy_core::FileMetadata
    pub(crate) fn files_metadata(&self) -> &Keyspace {
        &self.inner.files_metadata
    }

    /// Keyspace for raw file bytes keyed by `(actor_id,
    /// file_id)`. Blob-separated — files routinely run from
    /// kilobytes to many megabytes and the bytes are only
    /// loaded when the caller asks for `GET /files/{id}/content`.
    pub(crate) fn files_content(&self) -> &Keyspace {
        &self.inner.files_content
    }

    /// Keyspace for [`Run`] headers — short metadata blobs holding
    /// run state + per-policy / per-context refs.
    ///
    /// [`Run`]: super::super::runs::Run
    pub(crate) fn run_headers(&self) -> &Keyspace {
        &self.inner.run_headers
    }

    /// Keyspace for per-document run bodies — one entry per
    /// `(actor_id, run_id, doc_id)` carrying that document's
    /// recognized entities + reviewer overrides. Bytes (both
    /// input and post-apply redacted output) live in the
    /// [`files_metadata`](Self::files_metadata) /
    /// [`files_content`](Self::files_content) keyspaces; the
    /// row's [`RunDocument::input_file_id`] and
    /// [`output_file_id`](RunDocument::output_file_id) point at
    /// them.
    ///
    /// [`RunDocument::input_file_id`]: super::super::runs::RunDocument::input_file_id
    /// [`output_file_id`]: super::super::runs::RunDocument::output_file_id
    pub(crate) fn run_docs(&self) -> &Keyspace {
        &self.inner.run_docs
    }

    /// Flush all pending writes to disk. The engine HTTP layer
    /// calls this on graceful shutdown.
    pub fn sync(&self) -> Result<()> {
        self.inner.db.sync()
    }
}
