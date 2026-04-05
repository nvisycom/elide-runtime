//! Audio-modality entity location.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Overlap;
use crate::math::TimeSpan;

/// Location of an entity within an audio stream.
#[derive(Debug, Clone, PartialEq, Builder)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "AudioLocationBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct AudioLocation {
    /// Time interval of the entity.
    pub time_span: TimeSpan,
    /// Text extracted from the audio segment (e.g. via STT).
    ///
    /// Populated during extraction; skipped in serialization to prevent
    /// sensitive data from appearing in API responses.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing)]
    pub extracted_text: Option<String>,
    /// Speaker identifier from diarization.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
    /// Links this entity to a specific audio document.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_id: Option<Uuid>,
}

impl AudioLocation {
    /// Create a new [`AudioLocationBuilder`].
    pub fn builder() -> AudioLocationBuilder {
        AudioLocationBuilder::default()
    }
}

impl Overlap for AudioLocation {
    fn overlaps(&self, other: &Self) -> bool {
        self.time_span.overlaps(&other.time_span)
    }
}
