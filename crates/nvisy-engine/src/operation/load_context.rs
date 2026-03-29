//! Load context operation.
//!
//! Runs at **phase 0** alongside [`ImportFile`]. Produces context
//! references (UUIDs) that point into the run-wide [`ContextMap`]
//! pre-loaded on [`SharedContext`].
//!
//! [`ImportFile`]: crate::operation::ImportFile
//! [`ContextMap`]: nvisy_ontology::context::ContextMap
//! [`SharedContext`]: crate::operation::context::SharedContext

use nvisy_core::Result;
use nvisy_ontology::context::Contexts;
use nvisy_ontology::workflow::LoadContext as LoadContextCfg;
use uuid::Uuid;

use crate::operation::Operation;
use crate::operation::context::ParallelContext;

const TARGET: &str = "nvisy_engine::op::load_context";

/// Produces context references for downstream operations.
///
/// The actual context data is pre-loaded into the run-wide
/// [`ContextMap`] on [`SharedContext`]. This operation simply
/// records which context IDs this node contributes.
///
/// [`ContextMap`]: nvisy_ontology::context::ContextMap
/// [`SharedContext`]: crate::operation::context::SharedContext
pub struct LoadContext {
    context_ids: Vec<Uuid>,
}

impl LoadContext {
    /// Create from graph config.
    pub fn new(cfg: &LoadContextCfg) -> Self {
        tracing::debug!(target: TARGET, count = cfg.context_ids.len(), "load context references");
        Self {
            context_ids: cfg.context_ids.clone(),
        }
    }
}

impl Operation for LoadContext {
    type Input = ParallelContext<Contexts>;
    type Output = ParallelContext<Contexts>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input
            .parallel_map(|mut contexts| async {
                for &id in &self.context_ids {
                    contexts.push(id);
                }
                Ok(contexts)
            })
            .await
    }
}
