//! Audio-modality entity location.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Mergeable, Overlap};
use crate::primitive::TimeSpan;

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
    /// Create an [`AudioLocation`] covering `time_span`, with no
    /// speaker attribution or audio-document link. Use [`builder`]
    /// when `speaker_id` or `audio_id` need to be set.
    ///
    /// [`builder`]: Self::builder
    pub fn new(time_span: TimeSpan) -> Self {
        Self {
            time_span,
            speaker_id: None,
            audio_id: None,
        }
    }

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

impl Mergeable for AudioLocation {
    /// Merge two audio locations by unioning time spans when their
    /// `audio_id` and `speaker_id` match. Different speakers or
    /// different documents cannot merge.
    fn try_merge(self, other: Self) -> Option<Self> {
        if self.audio_id != other.audio_id || self.speaker_id != other.speaker_id {
            return None;
        }
        Some(Self {
            time_span: self.time_span.union(&other.time_span),
            speaker_id: self.speaker_id,
            audio_id: self.audio_id,
        })
    }
}
