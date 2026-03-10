//! Detection model identity and provenance.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Provenance or licensing classification of a detection model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// Model name (e.g. `"spacy-en-core-web-lg"`, `"gpt-4"`).
    pub name: String,
    /// Provenance / licensing classification.
    pub kind: ModelKind,
    /// Model version string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl ModelInfo {
    /// Create a new model info with the given name and kind.
    pub fn new(name: impl Into<String>, kind: ModelKind) -> Self {
        Self {
            name: name.into(),
            kind,
            version: None,
        }
    }

    /// Set the model version.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }
}
