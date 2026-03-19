//! Save context node configuration.
//!
//! [`SaveContext`] nodes run at phase 6 alongside [`ExportFile`] nodes,
//! persisting selected contexts from the envelope back to the registry.
//!
//! [`ExportFile`]: crate::graph::ExportFile

use nvisy_core::Result;
use nvisy_ontology::context::Contexts;
use nvisy_registry::Registry;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Configuration for the [`SaveContext`] graph node.
///
/// Specifies which contexts from the envelope should be persisted
/// back to the registry. Only contexts whose UUID appears in
/// `context_ids` are saved.
///
/// [`SaveContext`]: crate::graph::GraphNodeKind::SaveContext
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Validate, Serialize, Deserialize, JsonSchema)]
pub struct SaveContext {
    /// Context identifiers to persist to the registry.
    /// Must contain at least one.
    #[validate(length(min = 1, message = "save_context requires at least one context_id"))]
    pub context_ids: Vec<Uuid>,
}

impl SaveContext {
    /// Save matching contexts to the registry. Returns the number saved.
    pub async fn save(
        &self,
        registry: &Registry,
        actor_id: Uuid,
        contexts: &Contexts,
    ) -> Result<usize> {
        let mut saved = 0usize;
        for &id in &self.context_ids {
            if let Some(context) = contexts.get(&id) {
                registry.register_context(actor_id, context.clone()).await?;
                saved += 1;
            }
        }
        Ok(saved)
    }
}
