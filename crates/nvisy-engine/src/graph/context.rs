//! Context action configurations: load, save, and generate.

use nvisy_core::Error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuration for the [`LoadContext`](super::GraphNodeKind::LoadContext) action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LoadContext {
    /// Context identifiers to load.
    pub context_ids: Vec<Uuid>,
}

impl LoadContext {
    /// Validates that at least one context ID is specified.
    pub fn validate(&self) -> Result<(), Error> {
        if self.context_ids.is_empty() {
            return Err(Error::validation(
                "load_context requires at least one context id",
                "compiler",
            ));
        }
        Ok(())
    }
}

/// Configuration for the [`SaveContext`](super::GraphNodeKind::SaveContext) action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SaveContext {
    /// Context identifiers to persist.
    pub context_ids: Vec<Uuid>,
}

impl SaveContext {
    /// Validates that at least one context ID is specified.
    pub fn validate(&self) -> Result<(), Error> {
        if self.context_ids.is_empty() {
            return Err(Error::validation(
                "save_context requires at least one context id",
                "compiler",
            ));
        }
        Ok(())
    }
}

/// Configuration for the [`GenerateContext`](super::GraphNodeKind::GenerateContext) action.
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
