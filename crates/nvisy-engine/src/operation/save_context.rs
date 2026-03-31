//! Save context operation.
//!
//! Runs at **phase 6** alongside [`ExportFile`]. Persists selected
//! contexts from the cache to the registry.
//!
//! [`ExportFile`]: crate::operation::ExportFileOp

use std::sync::Arc;

use nvisy_core::Result;
use nvisy_ontology::workflow::SaveContext;
use uuid::Uuid;

use crate::operation::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::save_context";

/// Persists selected contexts from the cache to the registry.
///
/// Registry, actor identity, and context cache are read from the
/// envelope's [`SharedData`] at execution time.
///
/// [`SharedData`]: crate::operation::envelope::SharedData
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
            if !envelope.contexts.contains(&id) {
                tracing::trace!(target: TARGET, %id, "context not referenced by envelope, skipping");
                continue;
            }
            match shared.context_cache.get(&id).await {
                Some(context) => {
                    shared
                        .registry
                        .register_context(shared.actor_id, Arc::unwrap_or_clone(context))
                        .await?;
                    saved += 1;
                }
                None => {
                    tracing::trace!(target: TARGET, %id, "context not found in cache, skipping");
                }
            }
        }
        tracing::debug!(target: TARGET, saved, total = self.context_ids.len(), "persisted contexts");
        Ok(())
    }
}
