//! [`Audio`] modality marker, [`AudioLocation`] coordinate type, and
//! the [`AudioExtraction`] provenance enum.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Modality, Overlap};
use crate::entity::ModelProvenance;
use crate::primitive::TimeSpan;

/// Audio modality marker (zero-sized).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Audio;

impl Modality for Audio {
    type Location = AudioLocation;
}

/// A time interval within audio content.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AudioLocation {
    /// Time interval.
    pub time_span: TimeSpan,
    /// Speaker identifier from diarization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
    /// Links this interval to a specific audio document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_id: Option<Uuid>,
}

impl AudioLocation {
    /// Create an [`AudioLocation`] covering `time_span`, with no
    /// speaker attribution or audio-document link.
    pub fn new(time_span: TimeSpan) -> Self {
        Self {
            time_span,
            speaker_id: None,
            audio_id: None,
        }
    }
}

impl Overlap for AudioLocation {
    /// Two audio intervals overlap only when they target the same
    /// stream (matching `audio_id`) on the same `speaker_id` and
    /// their time spans intersect. Two voices captured on the same
    /// time interval are physically distinct redaction targets
    /// (source separation can suppress one speaker while keeping
    /// another), so different speaker IDs are not treated as
    /// overlapping even when the time spans intersect.
    fn overlaps(&self, other: &Self) -> bool {
        self.audio_id == other.audio_id
            && self.speaker_id == other.speaker_id
            && self.time_span.overlaps(&other.time_span)
    }
}

/// How a [`Document<Audio>`]'s content was produced.
///
/// [`Pending`] is the importer-time placeholder before any extractor
/// has run; the extractor stage replaces it with the concrete variant
/// carrying the backend's [`ModelProvenance`].
///
/// [`Document<Audio>`]: # "carrier owned by nvisy-document"
/// [`Pending`]: Self::Pending
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AudioExtraction {
    /// No extractor has run yet. Importer stamps this; the STT or
    /// diarization backend replaces it once samples are processed.
    Pending,
    /// Speech-to-text transcription: audio samples converted into
    /// text segments.
    Transcription(ModelProvenance),
    /// Speaker diarization: audio segmented by speaker identity
    /// before recognition attributes utterances.
    Diarization(ModelProvenance),
}
