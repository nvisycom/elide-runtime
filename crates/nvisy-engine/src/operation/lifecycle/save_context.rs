//! Persist contexts back to the registry.

use nvisy_core::Result;
use nvisy_ontology::context::Contexts;
use nvisy_registry::Registry;
use uuid::Uuid;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::save_context";

/// Saves selected contexts from the envelope back to the registry.
///
/// Only contexts whose `source.as_uuid()` appears in `context_ids` are
/// persisted. The operation is a side effect — the envelope is not modified.
pub struct SaveContext {
    actor_id: Uuid,
    context_ids: Vec<Uuid>,
    registry: Registry,
}

impl SaveContext {
    pub fn new(actor_id: Uuid, context_ids: Vec<Uuid>, registry: Registry) -> Self {
        Self { actor_id, context_ids, registry }
    }

    async fn persist(&self, contexts: Contexts) -> Result<()> {
        let mut saved = 0usize;
        for context in &contexts.contexts {
            let context_id = context.source.as_uuid();
            if self.context_ids.contains(&context_id) {
                self.registry
                    .register_context(self.actor_id, context.clone())
                    .await?;
                saved += 1;
            }
        }
        tracing::debug!(target: TARGET, saved, total = self.context_ids.len(), "saved contexts to registry");
        Ok(())
    }
}

impl Operation for SaveContext {
    type Input = ParallelContext<Contexts>;
    type Output = ParallelContext<()>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.persist(data)).await
    }
}
