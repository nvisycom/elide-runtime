//! Load context operation.
//!
//! Runs at **phase 0** alongside [`ImportFile`]. Produces context
//! references (UUIDs) that point into the run-wide [`ContextMap`]
//! pre-loaded on [`SharedData`].
//!
//! [`ImportFile`]: crate::operation::ImportFileOp
//! [`ContextMap`]: nvisy_ontology::context::ContextMap
//! [`SharedData`]: crate::operation::envelope::SharedData

use nvisy_core::Result;
use nvisy_ontology::workflow::LoadContext;
use uuid::Uuid;

use crate::operation::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::load_context";

/// Produces context references for downstream operations.
///
/// The actual context data is pre-loaded into the run-wide
/// [`ContextMap`] on [`SharedData`]. This operation simply
/// records which context IDs this node contributes.
///
/// [`ContextMap`]: nvisy_ontology::context::ContextMap
/// [`SharedData`]: crate::operation::envelope::SharedData
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
