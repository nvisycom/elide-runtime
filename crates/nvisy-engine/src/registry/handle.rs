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
const KS_RUN_HEADERS: &str = "run_headers";
const KS_RUN_DOCS: &str = "run_docs";
const KS_RUN_ARTIFACTS: &str = "run_artifacts";
const KS_RUN_INPUTS: &str = "run_inputs";

/// Cheaply-cloneable handle over the engine's fjall keyspaces.
#[derive(Clone)]
pub struct RegistryHandle {
    inner: Arc<RegistryInner>,
}

struct RegistryInner {
    db: Database,
    policies: Keyspace,
    contexts: Keyspace,
    run_headers: Keyspace,
    run_docs: Keyspace,
    run_artifacts: Keyspace,
    run_inputs: Keyspace,
}

impl RegistryHandle {
    /// Open (or create) the engine database at `path` and pre-open
    /// every keyspace. Idempotent — re-opening an existing database
    /// at the same path returns a handle to the same data.
    pub fn open(path: &Path) -> Result<Self> {
        let db = Database::open_at(path)?;
        let policies = db.open_blob_keyspace(KS_POLICIES)?;
        let contexts = db.open_blob_keyspace(KS_CONTEXTS)?;
        let run_headers = db.open_keyspace(KS_RUN_HEADERS)?;
        let run_docs = db.open_blob_keyspace(KS_RUN_DOCS)?;
        let run_artifacts = db.open_blob_keyspace(KS_RUN_ARTIFACTS)?;
        let run_inputs = db.open_blob_keyspace(KS_RUN_INPUTS)?;
        Ok(Self {
            inner: Arc::new(RegistryInner {
                db,
                policies,
                contexts,
                run_headers,
                run_docs,
                run_artifacts,
                run_inputs,
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

    /// Keyspace for [`Run`] headers — short metadata blobs holding
    /// run state + per-policy / per-context refs.
    ///
    /// [`Run`]: super::super::runs::Run
    pub(crate) fn run_headers(&self) -> &Keyspace {
        &self.inner.run_headers
    }

    /// Keyspace for per-document run bodies — one entry per
    /// `(actor_id, run_id, doc_id)` carrying that document's
    /// recognized entities + reviewer overrides. The body does
    /// **not** carry the post-apply redacted bytes; those live in
    /// [`run_artifacts`](Self::run_artifacts) so this body stays
    /// JSON-cheap to load for review surfaces.
    pub(crate) fn run_docs(&self) -> &Keyspace {
        &self.inner.run_docs
    }

    /// Keyspace for per-document post-apply artifacts (the
    /// redacted file bytes). One entry per
    /// `(actor_id, run_id, doc_id)` containing raw bytes. Blob-
    /// separated; lazily loaded only when the caller asks for the
    /// redacted output.
    pub(crate) fn run_artifacts(&self) -> &Keyspace {
        &self.inner.run_artifacts
    }

    /// Keyspace for original per-document input bytes. One entry
    /// per `(actor_id, run_id, doc_id)` containing raw bytes. The
    /// caller hands input bytes once at [`start`](super::super::runs::start);
    /// apply re-reads them from here (the codec needs the
    /// original bytes to re-decode).
    pub(crate) fn run_inputs(&self) -> &Keyspace {
        &self.inner.run_inputs
    }

    /// Flush all pending writes to disk. The engine HTTP layer
    /// calls this on graceful shutdown.
    pub fn sync(&self) -> Result<()> {
        self.inner.db.sync()
    }
}
