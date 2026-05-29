//! Audio modality.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AudioExtraction, Mergeable, Modality, ModalityBlock, Overlap};
use crate::policy::AudioStrategy;
use crate::primitive::{LanguageDetection, TimeSpan};

/// A time interval within audio content.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Audio {
    /// Time interval.
    pub time_span: TimeSpan,
    /// Speaker identifier from diarization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
    /// Links this interval to a specific audio document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_id: Option<Uuid>,
}

impl Audio {
    /// Create an [`Audio`] covering `time_span`, with no speaker
    /// attribution or audio-document link.
    pub fn new(time_span: TimeSpan) -> Self {
        Self {
            time_span,
            speaker_id: None,
            audio_id: None,
        }
    }
}

impl Modality for Audio {
    type Block = AudioBlock;
    type Extraction = AudioExtraction;
    type Metadata = AudioMetadata;
    type MethodTag = crate::policy::AudioMethodTag;
    /// Audio audits record only which method ran; the substitution
    /// is a binary sample transform whose parameters live on
    /// `AudioStrategy`.
    type Replacement = crate::policy::AudioMethodTag;
    type Strategy = AudioStrategy;

    fn default_method_dominance() -> &'static [Self::MethodTag] {
        // Silence is the only Partial-profile method; Remove is
        // Irrecoverable and never ties with Silence.
        &[crate::policy::AudioMethodTag::Silence]
    }
}

/// Per-modality block payload for [`Audio`].
/// [`Speech`] carries the transcript text and optional speaker;
/// per-word source spans live on the wrapping [`Block<Audio>`].
/// [`Silence`] carries no payload. Every variant carries the segment
/// `time_span`.
///
/// [`Speech`]: Self::Speech
/// [`Silence`]: Self::Silence
/// [`Block<Audio>`]: crate::document::Block
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AudioBlock {
    /// A transcribed speech segment (typically one speaker turn).
    Speech {
        time_span: TimeSpan,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speaker_id: Option<String>,
    },
    /// A silence or non-speech segment surfaced for completeness.
    Silence { time_span: TimeSpan },
}

impl AudioBlock {
    /// Segment time interval.
    pub fn time_span(&self) -> &TimeSpan {
        match self {
            Self::Speech { time_span, .. } | Self::Silence { time_span } => time_span,
        }
    }

    /// Transcribed text for [`Speech`], `None` for silence.
    ///
    /// [`Speech`]: Self::Speech
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Speech { text, .. } => Some(text),
            Self::Silence { .. } => None,
        }
    }
}

impl ModalityBlock for AudioBlock {
    fn scan_text(&self) -> Option<&str> {
        self.text()
    }
}

/// Document-level metadata for [`Document<Audio>`].
///
/// [`Document<Audio>`]: crate::document::Document
// TODO(#226): wire an importer that stamps `extraction` (Transcription
// or Diarization). No audio importer exists today; the field is
// type-required so that lands as a real value the moment one does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AudioMetadata {
    /// How this document's audio content was processed (STT
    /// transcription, speaker diarization).
    pub extraction: AudioExtraction,
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

impl From<AudioExtraction> for AudioMetadata {
    /// Build [`AudioMetadata`] carrying only the importer-known
    /// extraction tag. Languages, sample rate, and channel count
    /// start empty; downstream stages fill them in.
    fn from(extraction: AudioExtraction) -> Self {
        Self {
            extraction,
            languages: Vec::new(),
            sample_rate_hz: None,
            channels: None,
        }
    }
}

impl Overlap for Audio {
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

impl Mergeable for Audio {
    /// Merge two audio intervals by unioning time spans when their
    /// `audio_id` and `speaker_id` match. Different speakers or
    /// different documents cannot merge.
    fn try_merge(self, other: Self) -> Result<Self, (Self, Self)> {
        if self.audio_id != other.audio_id || self.speaker_id != other.speaker_id {
            return Err((self, other));
        }
        Ok(Self {
            time_span: self.time_span.union(&other.time_span),
            speaker_id: self.speaker_id,
            audio_id: self.audio_id,
        })
    }
}
