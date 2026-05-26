//! Audio modality.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Mergeable, Modality, Overlap};
use crate::primitive::{LanguageDetection, TimeSpan};

/// A time interval within audio content.
#[derive(Debug, Clone, PartialEq, Builder)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "AudioBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct Audio {
    /// Time interval.
    pub time_span: TimeSpan,
    /// Speaker identifier from diarization.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
    /// Links this interval to a specific audio document.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_id: Option<Uuid>,
}

impl Audio {
    /// Create an [`Audio`] covering `time_span`, with no speaker
    /// attribution or audio-document link. Use [`builder`] when
    /// `speaker_id` or `audio_id` need to be set.
    ///
    /// [`builder`]: Self::builder
    pub fn new(time_span: TimeSpan) -> Self {
        Self {
            time_span,
            speaker_id: None,
            audio_id: None,
        }
    }

    /// Create a new [`AudioBuilder`].
    pub fn builder() -> AudioBuilder {
        AudioBuilder::default()
    }
}

impl Modality for Audio {
    type BlockKind = AudioBlockKind;
    type Artefact = ();
    type Metadata = AudioMetadata;
}

/// Classification of a [`Block<Audio>`].
///
/// [`Block<Audio>`]: crate::document::Block
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AudioBlockKind {
    /// A transcribed speech segment (typically one speaker turn).
    #[default]
    Speech,
    /// A silence or non-speech segment surfaced for completeness.
    Silence,
}

/// Document-level metadata for [`Document<Audio>`].
///
/// [`Document<Audio>`]: crate::document::Document
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AudioMetadata {
    /// Languages detected (or asserted) for the transcribed content.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub languages: Vec<LanguageDetection>,
    /// Sample rate of the source audio, in Hz, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate_hz: Option<u32>,
    /// Number of channels in the source audio, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<u16>,
}

impl Overlap for Audio {
    fn overlaps(&self, other: &Self) -> bool {
        self.time_span.overlaps(&other.time_span)
    }
}

impl Mergeable for Audio {
    /// Merge two audio intervals by unioning time spans when their
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
