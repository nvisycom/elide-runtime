//! Persist selected envelope contexts back to the registry.

use nvisy_core::{Error, Result};
use nvisy_ontology::context::Contexts;
use uuid::Uuid;

use crate::operation::{DocumentEnvelope, Operation, ParallelContext, SharedContext};

const TARGET: &str = "nvisy_engine::op::save_context";

/// Saves contexts whose IDs match the configured list back to the registry.
pub struct SaveContext {
    actor_id: Uuid,
    context_ids: Vec<Uuid>,
    registry: nvisy_registry::Registry,
}

impl SaveContext {
    pub fn save(cfg: &crate::graph::SaveContext, shared: &SharedContext) -> Self {
        Self {
            actor_id: shared.actor_id,
            context_ids: cfg.context_ids.clone(),
            registry: shared.registry.clone(),
        }
    }

    pub(crate) async fn process(
        &self,
        envelope: DocumentEnvelope,
    ) -> Result<DocumentEnvelope, Error> {
        let mut saved = 0usize;
        for &id in &self.context_ids {
            if let Some(context) = envelope.contexts.get(&id) {
                self.registry
                    .register_context(self.actor_id, context.clone())
                    .await?;
                saved += 1;
            }
        }
        tracing::debug!(target: TARGET, saved, "saved contexts to registry");
        Ok(envelope)
    }
}

impl Operation for SaveContext {
    type Input = ParallelContext<Contexts>;
    type Output = ParallelContext<()>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input
            .parallel_map(|contexts| async move {
                let mut saved = 0usize;
                for &id in &self.context_ids {
                    if let Some(context) = contexts.get(&id) {
                        self.registry
                            .register_context(self.actor_id, context.clone())
                            .await?;
                        saved += 1;
                    }
                }
                tracing::debug!(target: TARGET, saved, "saved contexts via Operation");
                Ok(())
            })
            .await
    }
}
