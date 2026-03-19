//! Save context node configuration.
//!
//! [`SaveContext`] nodes run at phase 6 alongside [`ExportFile`] nodes,
//! persisting selected contexts from the envelope back to the registry.
//!
//! [`ExportFile`]: crate::graph::ExportFile

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
#[derive(Debug, Clone, PartialEq, Eq, Validate, Serialize, Deserialize, JsonSchema)]
pub struct SaveContext {
    /// Context identifiers to persist to the registry.
    /// Must contain at least one.
    #[validate(length(min = 1, message = "save_context requires at least one context_id"))]
    pub context_ids: Vec<Uuid>,
}
