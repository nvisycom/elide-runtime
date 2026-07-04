//! Voice biometric reference data.

use elide_core::primitive::TimeSpan;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Reference voice data for speaker identification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VoiceData {
    /// Id of the file holding the reference audio.
    pub audio_source: Uuid,
    /// Segment within the audio used for enrollment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment: Option<TimeSpan>,
    /// Base64-encoded speaker embedding / voiceprint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// Algorithm that produced the voiceprint (e.g. `"ecapa-tdnn"`, `"x-vector"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,
}

impl VoiceData {
    /// Create voice data pointing at a source audio.
    pub fn new(audio_source: Uuid) -> Self {
        Self {
            audio_source,
            segment: None,
            template: None,
            algorithm: None,
        }
    }

    /// Set the enrollment segment.
    pub fn with_segment(mut self, segment: TimeSpan) -> Self {
        self.segment = Some(segment);
        self
    }

    /// Set the encoded voiceprint.
    pub fn with_template(mut self, template: impl Into<String>) -> Self {
        self.template = Some(template.into());
        self
    }

    /// Set the extraction algorithm.
    pub fn with_algorithm(mut self, algorithm: impl Into<String>) -> Self {
        self.algorithm = Some(algorithm.into());
        self
    }
}
