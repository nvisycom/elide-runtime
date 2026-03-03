//! Data for AI-model inference operations.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::ModelInfo;

/// Data specific to AI-model operations (OCR, NER, transcription, etc.).
///
/// Duration and error are tracked on [`FileAuditEntry`](super::FileAuditEntry).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InferenceAction {
    /// Model used for this inference.
    pub model: ModelInfo,
    /// Input tokens consumed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    /// Output tokens produced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    /// Number of items processed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items_count: Option<u64>,
}

impl InferenceAction {
    /// Create a new inference action with the given model.
    pub fn new(model: ModelInfo) -> Self {
        Self {
            model,
            input_tokens: None,
            output_tokens: None,
            items_count: None,
        }
    }

    /// Set the token counts.
    pub fn with_tokens(mut self, input: u64, output: u64) -> Self {
        self.input_tokens = Some(input);
        self.output_tokens = Some(output);
        self
    }

    /// Set the number of items processed.
    pub fn with_items_count(mut self, count: u64) -> Self {
        self.items_count = Some(count);
        self
    }
}
