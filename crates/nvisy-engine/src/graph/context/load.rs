//! Load context node configuration.
//!
//! [`LoadContext`] nodes run at phase 0 alongside [`ImportFile`] nodes,
//! loading reference-data contexts from the registry into the envelope
//! for use by downstream detection and redaction stages.
//!
//! [`ImportFile`]: crate::graph::ImportFile

use nvisy_core::Result;
use nvisy_ontology::context::Context;
use nvisy_registry::Registry;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Configuration for the [`LoadContext`] graph node.
///
/// Specifies which reference-data contexts to load from the registry.
/// Each context is identified by its UUID and will be attached to
/// every document envelope passing through this node.
///
/// [`LoadContext`]: crate::graph::GraphNodeKind::LoadContext
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Validate, Serialize, Deserialize, JsonSchema)]
pub struct LoadContext {
    /// Context identifiers to load from the registry.
    /// Must contain at least one.
    #[validate(length(min = 1, message = "load_context requires at least one context_id"))]
    pub context_ids: Vec<Uuid>,
}

impl LoadContext {
    /// Load all contexts from the registry by their configured IDs.
    pub async fn load(&self, registry: &Registry, actor_id: Uuid) -> Result<Vec<Context>> {
        let mut contexts = Vec::with_capacity(self.context_ids.len());
        for &id in &self.context_ids {
            let handle = registry.read_context(actor_id, id).await?;
            contexts.push(handle.context().await?);
        }
        Ok(contexts)
    }
}
