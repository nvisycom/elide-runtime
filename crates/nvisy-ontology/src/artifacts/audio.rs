//! Audio-modality artifacts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Full transcript produced by speech-to-text extraction.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Transcription {
    /// The transcribed text.
    pub text: String,
    /// BCP-47 language tag of the detected language, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Artifacts produced during processing of audio content.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AudioArtifacts {
    /// Speech-to-text transcription result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcription: Option<Transcription>,
}
