//! Data for AI-model inference operations.

use derive_builder::Builder;
use nvisy_ontology::entity::ModelInfo;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Data specific to AI-model operations (OCR, NER, transcription, etc.).
///
/// Duration and error are tracked on [`AuditEntry`](super::super::AuditEntry).
#[derive(Debug, Clone, Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "InferenceActionBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct InferenceAction {
    /// Model used for this inference.
    pub model: ModelInfo,
    /// Input tokens consumed.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    /// Output tokens produced.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    /// Number of items processed.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items_count: Option<u64>,
}
