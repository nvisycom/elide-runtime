//! Load reference-data contexts from the registry into the envelope.

use nvisy_core::{Error, Result};
use nvisy_ontology::context::Contexts;

use crate::operation::{DocumentEnvelope, SharedContext};

const TARGET: &str = "nvisy_engine::op::load_context";

/// Loads contexts from the registry and merges them into each
/// passing envelope. Context IDs are resolved at construction time.
pub struct LoadContext {
    loaded: Contexts,
}

impl LoadContext {
    /// Build from graph config and shared context.
    pub async fn load(cfg: &crate::graph::LoadContext, shared: &SharedContext) -> Result<Self> {
        let mut loaded = Contexts::new();
        for &id in &cfg.context_ids {
            if loaded.contains(&id) {
                continue;
            }
            let handle = shared.registry.read_context(shared.actor_id, id).await?;
            let context = handle.context().await?;
            loaded.insert(context);
        }
        tracing::debug!(target: TARGET, count = loaded.len(), "loaded contexts from registry");
        Ok(Self { loaded })
    }

    pub(crate) async fn process(
        &self,
        mut envelope: DocumentEnvelope,
    ) -> Result<DocumentEnvelope, Error> {
        for (id, ctx) in self.loaded.iter() {
            if !envelope.contexts.contains(id) {
                envelope.contexts.insert(ctx.clone());
            }
        }
        Ok(envelope)
    }
}
