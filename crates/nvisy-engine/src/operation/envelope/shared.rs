//! Run-wide shared state for pipeline operations.
//!
//! [`SharedData`] holds immutable run-wide state behind an `Arc` so
//! that every envelope and operation can cheaply access the same actor
//! identity, policies, and context cache.

use std::sync::Arc;

use nvisy_ontology::policy::{Policies, Policy};
use uuid::Uuid;

use crate::operation::encryption::SharedKeyProvider;
use crate::registry::Registry;

/// Immutable run-wide state shared across all envelopes via `Arc`.
///
/// Constructed once at the start of a pipeline run and stored on each
/// [`DocumentEnvelope`](crate::operation::DocumentEnvelope).
#[derive(Clone)]
pub struct SharedData {
    /// Unique identifier for this pipeline run.
    pub run_id: Uuid,
    /// Identity of the human or service account that initiated the run.
    pub actor_id: Uuid,
    /// Policies governing redaction behaviour.
    pub policies: Policies,
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
            policies: Policies::default(),
            registry,
            key_provider: SharedKeyProvider::default(),
        })
    }

    /// Attach a key provider for encryption/decryption operations.
    pub fn with_key_provider(mut self, provider: SharedKeyProvider) -> Self {
        self.key_provider = provider;
        self
    }

    /// Attach policies to this shared data.
    pub fn with_policies(mut self, policies: Policies) -> Self {
        self.policies = policies;
        self
    }

    /// Append a single policy.
    pub fn with_policy(mut self, policy: Policy) -> Self {
        self.policies.push(policy);
        self
    }
}

impl std::fmt::Debug for SharedData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedData")
            .field("run_id", &self.run_id)
            .field("actor_id", &self.actor_id)
            .finish_non_exhaustive()
    }
}
