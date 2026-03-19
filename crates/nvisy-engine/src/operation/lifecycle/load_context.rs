//! Load reference-data contexts from the registry.

use nvisy_core::Result;
use nvisy_ontology::context::Contexts;
use uuid::Uuid;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::load_context";

/// Loads contexts from the registry by their identifiers.
///
/// The context IDs and loaded data are captured at construction time.
/// At call time, the operation merges the pre-loaded contexts into the
/// input `Contexts`, returning the combined set.
pub struct LoadContext {
    loaded: Contexts,
}

impl LoadContext {
    /// Load contexts from the registry at construction time.
    /// Load contexts from the registry at construction time.
    ///
    /// Duplicate IDs are ignored — each context is loaded at most once.
    pub async fn connect(
        registry: &nvisy_registry::Registry,
        actor_id: Uuid,
        context_ids: &[Uuid],
    ) -> Result<Self> {
        let mut seen = std::collections::HashSet::with_capacity(context_ids.len());
        let mut contexts = Vec::with_capacity(context_ids.len());
        for &id in context_ids {
            if !seen.insert(id) {
                continue;
            }
            let handle = registry.read_context(actor_id, id).await?;
            let context = handle.context().await?;
            contexts.push(context);
        }
        tracing::debug!(target: TARGET, count = contexts.len(), "loaded contexts from registry");
        Ok(Self {
            loaded: Contexts { contexts },
        })
    }

    async fn merge(&self, mut existing: Contexts) -> Result<Contexts> {
        let existing_ids: std::collections::HashSet<Uuid> = existing
            .contexts
            .iter()
            .map(|c| c.source.as_uuid())
            .collect();
        for ctx in &self.loaded.contexts {
            if !existing_ids.contains(&ctx.source.as_uuid()) {
                existing.contexts.push(ctx.clone());
            }
        }
        Ok(existing)
    }
}

impl Operation for LoadContext {
    type Input = ParallelContext<Contexts>;
    type Output = ParallelContext<Contexts>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.merge(data)).await
    }
}
