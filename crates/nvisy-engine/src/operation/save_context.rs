//! Save context operation.
//!
//! Runs at **phase 6** alongside [`ExportFile`]. Persists selected
//! contexts from the envelope back to the registry.
//!
//! [`ExportFile`]: crate::operation::ExportFile

use nvisy_core::{Error, Result};
use nvisy_ontology::context::Contexts;
use nvisy_registry::Registry;
use uuid::Uuid;

use crate::graph::SaveContext as SaveContextCfg;
use crate::operation::{DocumentEnvelope, Operation, ParallelContext, SharedContext};

const TARGET: &str = "nvisy_engine::op::save_context";

/// Saves contexts whose IDs match the configured list back to the registry.
pub struct SaveContext {
    actor_id: Uuid,
    context_ids: Vec<Uuid>,
    registry: Registry,
}

impl SaveContext {
    /// Create from graph config and shared context.
    pub fn new(cfg: &SaveContextCfg, shared: &SharedContext) -> Self {
        Self {
            actor_id: shared.actor_id,
            context_ids: cfg.context_ids.clone(),
            registry: shared.registry.clone(),
        }
    }

    async fn persist(&self, contexts: &Contexts) -> Result<usize> {
        let mut saved = 0usize;
        for &id in &self.context_ids {
            if let Some(context) = contexts.get(&id) {
                self.registry
                    .register_context(self.actor_id, context.clone())
                    .await?;
                saved += 1;
            }
        }
        Ok(saved)
    }

    pub(crate) async fn process(
        &self,
        envelope: DocumentEnvelope,
    ) -> Result<DocumentEnvelope, Error> {
        let saved = self.persist(&envelope.contexts).await?;
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
                self.persist(&contexts).await?;
                Ok(())
            })
            .await
    }
}
