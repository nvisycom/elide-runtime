//! Text-modality artifacts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::primitive::LanguageTag;

/// Artifacts produced during processing of text content.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TextArtifacts {
    /// BCP-47 language tag of the detected language, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub language: Option<LanguageTag>,
    /// Total character count of the text content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char_count: Option<u32>,
}
