//! Data for AI-model inference operations.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::ModelInfo;

/// Data for AI-model operations (OCR, NER, transcription, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InferenceAction {
    /// Model used for this inference.
    pub model: ModelInfo,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Input tokens consumed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    /// Output tokens produced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    /// Number of items processed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items_count: Option<u64>,
    /// Error message if the operation failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl InferenceAction {
    /// Create a new inference action with the given model and duration.
    pub fn new(model: ModelInfo, duration: Duration) -> Self {
        Self {
            model,
            duration_ms: duration.as_millis() as u64,
            input_tokens: None,
            output_tokens: None,
            items_count: None,
            error: None,
        }
    }

    /// Wall-clock duration.
    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.duration_ms)
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

    /// Set an error message for a failed operation.
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }
}
