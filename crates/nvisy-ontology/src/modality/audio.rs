//! Audio modality.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Mergeable, Modality, Overlap};
use crate::document::Span;
use crate::primitive::{Confidence, LanguageDetection, TimeSpan};

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
    type Block = AudioBlock;
    type Metadata = AudioMetadata;
    type Strategy = crate::policy::AudioStrategy;
}

/// One segment of an audio document.
///
/// `kind` carries the segment variant (speech, silence) and its
/// payload; `time_span` is the segment's interval; `confidence` is
/// the recognition confidence for the segment as a whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AudioBlock {
    /// Variant-specific payload.
    #[serde(flatten)]
    pub kind: AudioBlockKind,
    /// Segment time interval.
    pub time_span: TimeSpan,
    /// Recognition confidence for the segment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,
}

/// Variants of [`AudioBlock`]. [`Speech`](Self::Speech) carries the
/// transcript text plus per-word spans and optional speaker;
/// [`Silence`](Self::Silence) carries no payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AudioBlockKind {
    /// A transcribed speech segment (typically one speaker turn).
    Speech {
        text: String,
        spans: Vec<Span<Audio>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speaker_id: Option<String>,
    },
    /// A silence or non-speech segment surfaced for completeness.
    Silence,
}

impl AudioBlock {
    /// Transcribed text for [`Speech`](AudioBlockKind::Speech), `None`
    /// for silence.
    pub fn text(&self) -> Option<&str> {
        self.kind.text()
    }

    /// Per-word spans for [`Speech`](AudioBlockKind::Speech), empty
    /// for silence.
    pub fn spans(&self) -> &[Span<Audio>] {
        self.kind.spans()
    }
}

impl AudioBlockKind {
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Speech { text, .. } => Some(text),
            Self::Silence => None,
        }
    }

    pub fn spans(&self) -> &[Span<Audio>] {
        match self {
            Self::Speech { spans, .. } => spans,
            Self::Silence => &[],
        }
    }
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
