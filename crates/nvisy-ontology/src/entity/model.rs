//! Detection model identity and provenance.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Provenance or licensing classification of a detection model.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    /// Open-source model (e.g. spaCy, Hugging Face community models).
    OpenSource,
    /// Proprietary model (e.g. vendor-specific NER).
    Proprietary,
    /// Model accessed through a third-party API gateway.
    Gateway,
    /// Self-hosted model served behind an internal endpoint.
    SelfHosted,
}

/// Identity and version of the model used for detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(JsonSchema)]
pub struct ModelInfo {
    /// Model name (e.g. `"spacy-en-core-web-lg"`, `"gpt-4"`).
    pub name: String,
    /// Provenance / licensing classification.
    pub kind: ModelKind,
    /// Model version string.
    pub version: String,
}
