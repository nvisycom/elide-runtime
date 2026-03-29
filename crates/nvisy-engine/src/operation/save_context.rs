//! Save context operation.
//!
//! Runs at **phase 6** alongside [`ExportFile`]. Persists selected
//! contexts from the run-wide context map to the registry.
//!
//! [`ExportFile`]: crate::operation::ExportFile

use nvisy_core::Result;
use nvisy_ontology::context::Contexts;
use nvisy_ontology::workflow::SaveContext;
use uuid::Uuid;

use crate::operation::Operation;
use crate::operation::context::ParallelContext;

const TARGET: &str = "nvisy_engine::op::save_context";

/// Persists selected contexts from the run-wide context map to the registry.
///
/// Registry, actor identity, and context map are read from the
/// [`SharedContext`] at call time — only the configured context IDs
/// are stored on the struct.
///
/// [`SharedContext`]: crate::operation::context::SharedContext
pub struct SaveContextOp {
    context_ids: Vec<Uuid>,
}

impl SaveContextOp {
    /// Create from graph config.
    pub fn new(cfg: &SaveContext) -> Self {
        Self {
            context_ids: cfg.context_ids.clone(),
        }
    }
}

impl Operation for SaveContextOp {
    type Input = ParallelContext<Contexts>;
    type Output = ParallelContext<()>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        tracing::debug!(target: TARGET, "saving contexts to registry");
        let shared = input.shared.clone();
        input
            .parallel_map(|contexts| async move {
                let mut saved = 0usize;
                for &id in &self.context_ids {
                    if contexts.contains(&id)
                        && let Some(context) = shared.context_map.get(&id)
                    {
                        shared
                            .registry
                            .register_context(shared.actor_id, context.clone())
                            .await?;
                        saved += 1;
                    }
                }
                tracing::debug!(target: TARGET, saved, "persisted contexts");
                Ok(())
            })
            .await
    }
}
