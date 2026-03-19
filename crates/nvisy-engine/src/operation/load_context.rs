//! Load reference-data contexts from the registry into the envelope.

use std::collections::HashSet;

use nvisy_core::{Error, Result};
use nvisy_ontology::context::Contexts;
use uuid::Uuid;

use crate::operation::{DocumentEnvelope, NodeHandler, SharedContext};

const TARGET: &str = "nvisy_engine::op::load_context";

/// Loads contexts from the registry and merges them into each
/// passing envelope. Context IDs are resolved at construction time.
pub struct LoadContext {
    loaded: Contexts,
}

impl LoadContext {
    /// Build from graph config and shared context.
    pub async fn connect(cfg: &crate::graph::LoadContext, shared: &SharedContext) -> Result<Self> {
        let mut seen = HashSet::with_capacity(cfg.context_ids.len());
        let mut contexts = Vec::with_capacity(cfg.context_ids.len());
        for &id in &cfg.context_ids {
            if !seen.insert(id) {
                continue;
            }
            let handle = shared.registry.read_context(shared.actor_id, id).await?;
            let context = handle.context().await?;
            contexts.push(context);
        }
        tracing::debug!(target: TARGET, count = contexts.len(), "loaded contexts from registry");
        Ok(Self {
            loaded: Contexts { contexts },
        })
    }
}

#[async_trait::async_trait]
impl NodeHandler for LoadContext {
    async fn handle(&self, mut envelope: DocumentEnvelope) -> Result<DocumentEnvelope, Error> {
        let existing_ids: HashSet<Uuid> = envelope
            .contexts
            .contexts
            .iter()
            .map(|c| c.source.as_uuid())
            .collect();
        for ctx in &self.loaded.contexts {
            if !existing_ids.contains(&ctx.source.as_uuid()) {
                envelope.contexts.contexts.push(ctx.clone());
            }
        }
        Ok(envelope)
    }
}
