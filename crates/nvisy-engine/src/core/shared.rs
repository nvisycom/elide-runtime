//! Run-wide shared state for pipeline operations.
//!
//! [`SharedData`] holds immutable run-wide state behind an `Arc` so
//! that every envelope and operation can cheaply access the same actor
//! identity, policies, and context cache.
//!
//! Policies are typed per modality; the [`PolicyStore`] in this struct
//! holds the per-modality stacks heterogeneously, so a
//! `DocumentEnvelope<M>` pulls its stack with
//! `shared.policies.resolve::<M>(...)`.

use std::fmt;
use std::sync::Arc;

use nvisy_codec::CodecRegistry;
use nvisy_core::entity::EntityLabelCatalog;
use uuid::Uuid;

use super::PolicyStore;
use crate::core::ingestion::encryption::SharedKeyProvider;
use crate::registry::Registry;

/// Immutable run-wide state shared across all envelopes via `Arc`.
pub struct SharedData {
    /// Unique identifier for this pipeline run.
    pub run_id: Uuid,
    /// Identity of the human or service account that initiated the run.
    pub actor_id: Uuid,
    /// Per-modality policy stacks built at submission time. Held as
    /// `Arc<PolicyStore>` so detection and redaction passes share
    /// the same store without rebuilding it.
    pub policies: Arc<PolicyStore>,
    /// Per-request entity-label catalog, unioned from every
    /// submitted policy's [`Policy::labels`]. Drives recognizer
    /// dispatch (NER label list, pattern filtering) and selector
    /// tag matching.
    ///
    /// [`Policy::labels`]: crate::policy::Policy::labels
    pub catalog: Arc<EntityLabelCatalog>,
    /// Content and context storage.
    pub registry: Registry,
    /// Codec registry resolving file extensions / content types to
    /// the appropriate per-format loader. Importers call into this
    /// to decode raw bytes into a typed [`DocumentHandle<M>`].
    ///
    /// Built once at engine construction with
    /// [`CodecRegistry::with_builtin`] so every importer in the run
    /// shares the same set of registered formats.
    ///
    /// [`DocumentHandle<M>`]: nvisy_codec::DocumentHandle
    pub codec_registry: CodecRegistry,
    /// Key provider for encryption/decryption.
    pub key_provider: SharedKeyProvider,
}

impl fmt::Debug for SharedData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedData")
            .field("run_id", &self.run_id)
            .field("actor_id", &self.actor_id)
            .finish_non_exhaustive()
    }
}
