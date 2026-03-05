//! Voice biometric reference data.

use nvisy_core::math::TimeSpan;
use nvisy_core::path::ContentSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Reference voice data for speaker identification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VoiceData {
    /// Source pointer to the reference audio.
    pub audio_source: ContentSource,
    /// Segment within the audio used for enrollment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment: Option<TimeSpan>,
    /// Audio format hint (e.g. `"wav"`, `"flac"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Base64-encoded speaker embedding / voiceprint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// Algorithm that produced the voiceprint (e.g. `"ecapa-tdnn"`, `"x-vector"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,
}

impl VoiceData {
    /// Create voice data pointing at a source audio.
    pub fn new(audio_source: ContentSource) -> Self {
        Self {
            audio_source,
            segment: None,
            format: None,
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
