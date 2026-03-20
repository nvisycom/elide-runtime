//! Load context operation.
//!
//! Runs at **phase 0** alongside [`ImportFile`]. Loads reference-data
//! contexts from the registry by their configured IDs and merges them
//! into each passing envelope.
//!
//! [`ImportFile`]: crate::operation::ImportFile

use nvisy_core::{Error, Result};
use nvisy_ontology::context::Contexts;

use crate::graph::LoadContext as LoadContextCfg;
use crate::operation::{DocumentEnvelope, Operation, ParallelContext, SharedContext};

const TARGET: &str = "nvisy_engine::op::load_context";

/// Loads contexts from the registry and merges them into each
/// passing envelope. Context IDs are resolved at construction time.
pub struct LoadContext {
    loaded: Contexts,
}

impl LoadContext {
    /// Create from graph config and shared context.
    pub async fn new(cfg: &LoadContextCfg, shared: &SharedContext) -> Result<Self> {
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
        tracing::debug!(target: TARGET, loaded = self.loaded.len(), "merging contexts into envelope");
        for (id, ctx) in self.loaded.iter() {
            if !envelope.contexts.contains(id) {
                envelope.contexts.insert(ctx.clone());
            }
        }
        Ok(envelope)
    }
}

impl Operation for LoadContext {
    type Input = ParallelContext<Contexts>;
    type Output = ParallelContext<Contexts>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input
            .parallel_map(|mut existing| async move {
                for (id, ctx) in self.loaded.iter() {
                    if !existing.contains(id) {
                        existing.insert(ctx.clone());
                    }
                }
                Ok(existing)
            })
            .await
    }
}
