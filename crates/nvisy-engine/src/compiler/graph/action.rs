//! Action node definition with strongly-typed action variants.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The set of strongly-typed actions a pipeline node can perform.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// Run detection methods on the content.
    Detect,
    /// Transcribe audio content to text.
    Transcribe,
    /// Translate content between languages.
    Translate,
    /// Apply redaction instructions to the content.
    Redact,
    /// Evaluate policies against detected entities.
    Evaluate,
}

/// A transformation or detection step.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActionNode {
    /// The action this node performs.
    pub action: ActionKind,
}
