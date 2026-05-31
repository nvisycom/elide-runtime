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

use nvisy_ontology::modality::Modality;
use nvisy_ontology::policy::Policy;
use uuid::Uuid;

use super::PolicyStore;
use crate::ingestion::encryption::SharedKeyProvider;
use crate::ingestion::registry::Registry;

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
    /// Key provider for encryption/decryption.
    pub key_provider: SharedKeyProvider,
}

impl SharedData {
    /// Create a new shared data with the given run, actor, and registry.
    pub fn new(run_id: Uuid, actor_id: Uuid, registry: Registry) -> Arc<Self> {
        Arc::new(Self {
            run_id,
            actor_id,
            policies: PolicyStore::new(),
            registry,
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
    pub fn with_policy<M: Modality>(mut self, policy: Arc<Policy<M>>) -> Self {
        self.policies.insert(policy);
        self
    }

    /// Replace the policy stack for modality `M`.
    pub fn with_policies<M: Modality>(mut self, policies: Vec<Arc<Policy<M>>>) -> Self {
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
