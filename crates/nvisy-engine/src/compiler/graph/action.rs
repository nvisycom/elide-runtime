//! Action node definition with strongly-typed action variants.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The set of strongly-typed actions a pipeline node can perform.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// Run OCR before connected nodes.
    Ocr,
    /// Transcribe audio content to text.
    Transcribe,
    /// Run entity detection (NER, pattern, CV).
    Detect,
    /// Evaluate policies against detected entities.
    Evaluate,
    /// Apply redaction instructions to the content.
    Redact,
    /// Translate content or context between languages.
    Translate,
    /// Classify content and route to different outputs.
    Classify,
    /// Generate a summary and inject into context.
    Summarize,
    /// Emit a per-file audit record.
    Audit,
    /// Deliver to a target connection.
    Publish,
}

/// A transformation or detection step.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActionNode {
    /// The action this node performs.
    pub action: ActionKind,
}
