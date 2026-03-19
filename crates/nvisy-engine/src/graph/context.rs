//! Context action configurations: load, save, and generate.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Configuration for the [`LoadContext`] action.
///
/// [`LoadContext`]: super::GraphNodeKind::LoadContext
#[derive(Debug, Clone, PartialEq, Eq, Validate, Serialize, Deserialize, JsonSchema)]
pub struct LoadContext {
    /// Context identifiers to load. Must contain at least one.
    #[validate(length(min = 1, message = "load_context requires at least one context_id"))]
    pub context_ids: Vec<Uuid>,
}

/// Configuration for the [`SaveContext`] action.
///
/// [`SaveContext`]: super::GraphNodeKind::SaveContext
#[derive(Debug, Clone, PartialEq, Eq, Validate, Serialize, Deserialize, JsonSchema)]
pub struct SaveContext {
    /// Context identifiers to persist. Must contain at least one.
    #[validate(length(min = 1, message = "save_context requires at least one context_id"))]
    pub context_ids: Vec<Uuid>,
}

/// Configuration for the [`GenerateContext`] action.
///
/// [`GenerateContext`]: super::GraphNodeKind::GenerateContext
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct GenerateContext {
    /// Include a span-level summary in the generated context.
    #[serde(default)]
    pub summarization: bool,
    /// Include translated spans in the generated context.
    #[serde(default)]
    pub translation: bool,
    /// Include an audit record in the generated context.
    #[serde(default)]
    pub audit: bool,
}
