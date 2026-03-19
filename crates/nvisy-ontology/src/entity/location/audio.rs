//! Audio-modality entity location.

use nvisy_core::math::TimeSpan;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Overlap;

/// Location of an entity within an audio stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AudioLocation {
    /// Time interval of the entity.
    pub time_span: TimeSpan,
    /// Speaker identifier from diarization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
    /// Links this entity to a specific audio document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_id: Option<Uuid>,
}

impl Overlap for AudioLocation {
    fn overlaps(&self, other: &Self) -> bool {
        self.time_span.overlaps(&other.time_span)
    }
}
