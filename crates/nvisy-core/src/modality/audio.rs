//! [`Audio`] modality marker, [`AudioLocation`] coordinate type,
//! [`AudioData`] per-call payload, and [`AudioExtraction`] provenance
//! enum.

use bytes::Bytes;
use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Modality, Overlap};
use crate::entity::ModelProvenance;
use crate::primitive::TimeSpan;
use crate::redaction::AudioReplacement;

/// Audio modality marker (zero-sized).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Audio;

impl Modality for Audio {
    type Data = AudioData;
    type Extraction = AudioExtraction;
    type Location = AudioLocation;
    type Replacement = AudioReplacement;

    const KIND: super::ModalityKind = super::ModalityKind::Audio;
    const NAME: &'static str = "audio";
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

impl Eq for AudioLocation {}

impl Ord for AudioLocation {
    /// Lex order over `(time_span.start_us, time_span.end_us)`.
    /// `speaker_id` and `audio_id` are ignored.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.time_span.start_us, self.time_span.end_us)
            .cmp(&(other.time_span.start_us, other.time_span.end_us))
    }
}

impl PartialOrd for AudioLocation {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Per-call payload for [`Audio`] extractors.
///
/// Audio backends (STT, diarization) take encoded bytes plus an
/// optional original filename. No dimensions or sample-rate metadata
/// is carried at this layer — providers parse the container
/// themselves.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioData {
    /// Encoded audio bytes.
    pub bytes: Bytes,
    /// Original filename, when known. Threaded through to providers
    /// like OpenAI Whisper that key on the source filename's
    /// extension to pick a decoder.
    pub filename: Option<HipStr<'static>>,
}

impl AudioData {
    /// Construct with the encoded bytes; filename is initially unset.
    pub fn new(bytes: impl Into<Bytes>) -> Self {
        Self {
            bytes: bytes.into(),
            filename: None,
        }
    }

    /// Attach an original filename hint.
    pub fn with_filename(mut self, filename: impl Into<HipStr<'static>>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    /// Extension derived from [`filename`], or `"mp3"` when no
    /// filename is set or the filename has no extension.
    ///
    /// [`filename`]: Self::filename
    pub fn extension(&self) -> &str {
        self.filename
            .as_deref()
            .and_then(|name| name.rsplit_once('.'))
            .map(|(_, ext)| ext)
            .unwrap_or("mp3")
    }

    /// View the encoded bytes.
    pub fn as_bytes(&self) -> &Bytes {
        &self.bytes
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
/// [`Document<Audio>`]: # "carrier owned by nvisy-engine"
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
