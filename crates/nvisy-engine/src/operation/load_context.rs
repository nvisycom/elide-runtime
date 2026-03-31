//! Load context operation.
//!
//! Runs at **phase 0** alongside [`ImportFile`]. Records which context
//! IDs this node contributes to the envelope. The actual context data
//! is loaded into the engine's [`ContextCache`] at run start and
//! accessed on demand via `envelope.shared.context_cache`.
//!
//! [`ImportFile`]: crate::operation::ImportFileOp
//! [`ContextCache`]: crate::pipeline::cache::ContextCache

use nvisy_core::Result;
use nvisy_ontology::workflow::LoadContext;
use uuid::Uuid;

use crate::operation::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::load_context";

/// Records context references on the envelope for downstream operations.
///
/// The actual context data lives in the engine's [`ContextCache`],
/// accessed via `envelope.shared.context_cache`. This operation
/// records which context IDs this node contributes so that
/// downstream nodes (e.g. [`GenerateContextOp`]) know which
/// contexts are available.
///
/// [`ContextCache`]: crate::pipeline::cache::ContextCache
/// [`GenerateContextOp`]: crate::operation::GenerateContextOp
pub struct LoadContextOp {
    context_ids: Vec<Uuid>,
}

impl LoadContextOp {
    /// Create from graph config.
    pub fn new(cfg: &LoadContext) -> Self {
        tracing::debug!(target: TARGET, count = cfg.context_ids.len(), "load context references");
        Self {
            context_ids: cfg.context_ids.clone(),
        }
    }
}

impl Operation for LoadContextOp {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        for &id in &self.context_ids {
            envelope.contexts.push(id);
        }
        tracing::debug!(
            target: TARGET,
            added = self.context_ids.len(),
            total = envelope.contexts.len(),
            "loaded context references",
        );
        Ok(())
    }
}
