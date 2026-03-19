//! Persist selected envelope contexts back to the registry.

use nvisy_core::{Error, Result};
use uuid::Uuid;

use crate::operation::{DocumentEnvelope, NodeHandler, SharedContext};

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
}

#[async_trait::async_trait]
impl NodeHandler for SaveContext {
    async fn handle(&self, envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        let mut saved = 0usize;
        for context in &envelope.contexts.contexts {
            if self.context_ids.contains(&context.source.as_uuid()) {
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
