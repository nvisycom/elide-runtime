//! Audio-modality document shape: [`AudioBlock`], [`AudioMetadata`].

use nvisy_core::modality::AudioExtraction;
use nvisy_core::primitive::{LanguageDetection, TimeSpan};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::ModalityBlock;

/// Per-modality block payload for [`Audio`].
/// [`Speech`] carries the transcript text and optional speaker;
/// per-word source spans live on the wrapping [`Block<Audio>`].
/// [`Silence`] carries no payload. Every variant carries the segment
/// `time_span`.
///
/// [`Audio`]: nvisy_core::modality::Audio
/// [`Speech`]: Self::Speech
/// [`Silence`]: Self::Silence
/// [`Block<Audio>`]: crate::document::Block
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AudioBlock {
    /// A transcribed speech segment (typically one speaker turn).
    Speech {
        /// Segment time interval.
        time_span: TimeSpan,
        /// Transcribed text content.
        text: String,
        /// Speaker identifier from diarization, when available.
        speaker_id: Option<String>,
    },
    /// A silence or non-speech segment surfaced for completeness.
    Silence {
        /// Segment time interval.
        time_span: TimeSpan,
    },
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
