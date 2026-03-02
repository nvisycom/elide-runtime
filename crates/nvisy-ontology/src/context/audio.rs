//! Audio-modality reference data.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use nvisy_core::math::TimeSpan;
use nvisy_core::path::ContentSource;

/// Audio reference (voice sample, keyword, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AudioData {
    /// Source pointer to the reference audio.
    pub audio_source: ContentSource,
    /// Optional time segment within the audio.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment: Option<TimeSpan>,
}
