//! Load context node configuration.
//!
//! [`LoadContext`] nodes run at phase 0 alongside [`ImportFile`] nodes,
//! loading reference-data contexts from the registry into the envelope
//! for use by downstream detection and redaction stages.
//!
//! [`ImportFile`]: crate::graph::ImportFile

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
