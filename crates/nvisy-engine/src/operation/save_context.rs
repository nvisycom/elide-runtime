//! Save context operation.
//!
//! Runs at **phase 6** alongside [`ExportFile`]. Persists selected
//! contexts from the run-wide context map to the registry.
//!
//! [`ExportFile`]: crate::operation::ExportFileOp

use nvisy_core::Result;
use nvisy_ontology::workflow::SaveContext;
use uuid::Uuid;

use crate::operation::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::save_context";

/// Persists selected contexts from the run-wide context map to the registry.
///
/// Registry, actor identity, and context map are read from the
/// envelope's [`SharedData`] at execution time.
///
/// [`SharedData`]: crate::operation::context::SharedData
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
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        tracing::debug!(target: TARGET, "saving contexts to registry");
        let shared = &envelope.shared;
        let mut saved = 0usize;
        for &id in &self.context_ids {
            if envelope.contexts.contains(&id)
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
    }
}
