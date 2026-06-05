//! Run-wide shared state for pipeline operations.
//!
//! [`SharedData`] holds immutable run-wide state behind an `Arc` so
//! that every envelope and operation can cheaply access the same actor
//! identity, policies, and context cache.
//!
//! Policies are typed per modality; the [`PolicyStore`] in this struct
//! holds the per-modality stacks heterogeneously, so a
//! `DocumentEnvelope<M>` pulls its stack with
//! `shared.policies.get::<M>()`.

use std::fmt;
use std::sync::Arc;

use nvisy_codec::CodecRegistry;
use nvisy_formats::CodecRegistryExt;
use uuid::Uuid;

use super::PolicyStore;
use crate::modality::DocumentModality;
use crate::phases::ingestion::encryption::SharedKeyProvider;
use crate::phases::ingestion::registry::Registry;
use crate::policy::Policy;

/// Immutable run-wide state shared across all envelopes via `Arc`.
pub struct SharedData {
    /// Unique identifier for this pipeline run.
    pub run_id: Uuid,
    /// Identity of the human or service account that initiated the run.
    pub actor_id: Uuid,
    /// Per-modality policy stacks, in precedence order (index 0 highest).
    pub policies: PolicyStore,
    /// Content and context storage.
    pub registry: Registry,
    /// Codec registry resolving file extensions / content types to
    /// the appropriate per-format loader. Importers call into this
    /// to decode raw bytes into a typed [`DocumentHandle<M>`][dh].
    ///
    /// Built once at engine construction with
    /// [`CodecRegistry::builtins`] so every importer in the run
    /// shares the same set of registered formats.
    ///
    /// [dh]: nvisy_codec::DocumentHandle
    pub codec_registry: CodecRegistry,
    /// Key provider for encryption/decryption.
    pub key_provider: SharedKeyProvider,
}

impl SharedData {
    /// Create a new shared data with the given run, actor, and registry.
    /// The codec registry is preloaded with every built-in format the
    /// active feature set enables.
    pub fn new(run_id: Uuid, actor_id: Uuid, registry: Registry) -> Arc<Self> {
        Arc::new(Self {
            run_id,
            actor_id,
            policies: PolicyStore::new(),
            registry,
            codec_registry: CodecRegistry::builtins(),
            key_provider: SharedKeyProvider::default(),
        })
    }

    /// Attach a key provider for encryption/decryption operations.
    pub fn with_key_provider(mut self, provider: SharedKeyProvider) -> Self {
        self.key_provider = provider;
        self
    }

    /// Append a single policy for modality `M`. The policy is held
    /// as an [`Arc`] so it can also live in the registry's cross-run
    /// cache without copying.
    pub fn with_policy<M: DocumentModality>(mut self, policy: Arc<Policy<M>>) -> Self {
        self.policies.insert(policy);
        self
    }

    /// Replace the policy stack for modality `M`.
    pub fn with_policies<M: DocumentModality>(mut self, policies: Vec<Arc<Policy<M>>>) -> Self {
        self.policies.set::<M>(policies);
        self
    }
}

impl fmt::Debug for SharedData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedData")
            .field("run_id", &self.run_id)
            .field("actor_id", &self.actor_id)
            .finish_non_exhaustive()
    }
}
