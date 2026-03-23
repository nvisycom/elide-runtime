//! Run-wide shared state for pipeline operations.
//!
//! [`SharedContext`] wraps an `Arc<SharedData>` so that every operation
//! in a pipeline run can cheaply access the same actor identity,
//! policies, and reference-data contexts without copying or threading
//! them through individual parameter structs.

use std::sync::Arc;

use derive_more::Deref;
use nvisy_ontology::context::ContextMap;
use nvisy_ontology::policy::{Policies, Policy};
use uuid::Uuid;

use crate::operation::encryption::SharedKeyProvider;
use crate::registry::Registry;

/// Immutable run-wide state behind an [`Arc`].
///
/// Constructed once at the start of a pipeline run and shared across
/// all operations via [`SharedContext`].
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
    /// Pre-loaded contexts for this pipeline run.
    pub context_map: ContextMap,
}

/// Cheaply-clonable handle to run-wide shared data.
///
/// Cloning is an `Arc` bump: every [`ParallelContext`] and
/// [`SequentialContext`] carries one of these. `Deref<Target =
/// SharedData>` gives direct field access (e.g. `ctx.shared.run_id`).
///
/// [`ParallelContext`]: super::ParallelContext
/// [`SequentialContext`]: super::SequentialContext
#[derive(Debug, Clone, Deref)]
pub struct SharedContext {
    #[deref]
    data: Arc<SharedData>,
}

impl SharedContext {
    /// Create a new shared context with the given run, actor, and registry.
    pub fn new(run_id: Uuid, actor_id: Uuid, registry: Registry) -> Self {
        Self {
            data: Arc::new(SharedData {
                run_id,
                actor_id,
                policies: Policies::default(),
                registry,
                key_provider: SharedKeyProvider::default(),
                context_map: ContextMap::new(),
            }),
        }
    }

    /// Attach a key provider for encryption/decryption operations.
    pub fn with_key_provider(mut self, provider: SharedKeyProvider) -> Self {
        Arc::make_mut(&mut self.data).key_provider = provider;
        self
    }

    /// Attach policies to this shared context.
    pub fn with_policies(mut self, policies: Policies) -> Self {
        Arc::make_mut(&mut self.data).policies = policies;
        self
    }

    /// Attach a pre-loaded context map.
    pub fn with_context_map(mut self, map: ContextMap) -> Self {
        Arc::make_mut(&mut self.data).context_map = map;
        self
    }

    /// Append a single policy.
    pub fn with_policy(mut self, policy: Policy) -> Self {
        Arc::make_mut(&mut self.data).policies.policies.push(policy);
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
