//! [`TranscribedSegment`] + [`TranscribedWord`]: the per-segment
//! transcription unit produced by an [`SttBackend`].
//!
//! [`SttBackend`]: super::SttBackend

use nvisy_core::primitive::{Confidence, LanguageTag, TimeSpan};

/// One transcription segment from an [`SttBackend`].
///
/// Carries a time interval, the recognised text, and a handful of
/// optional fields that providers may or may not fill in.
/// `speaker_id` is populated only by providers with diarization
/// (Deepgram, AssemblyAI, etc.) — OpenAI Whisper leaves it `None`.
/// `language` is populated by providers that emit per-segment language
/// detection (handy for code-switching audio).
///
/// [`SttBackend`]: super::SttBackend
#[derive(Debug, Clone, PartialEq)]
pub struct TranscribedSegment {
    /// Time interval (microsecond precision) within the source clip.
    pub time_span: TimeSpan,
    /// Speaker label, if the backend performed diarization.
    pub speaker_id: Option<String>,
    /// Detected language for this segment, if reported.
    pub language: Option<LanguageTag>,
    /// Recognised text for this segment.
    pub text: String,
    /// Backend confidence in the segment, if reported.
    pub confidence: Option<Confidence>,
    /// Word-level breakdown, if the backend emitted one. Empty
    /// otherwise.
    pub words: Vec<TranscribedWord>,
}

/// One word inside a [`TranscribedSegment`].
///
/// Populated by backends that emit word-level timestamps (OpenAI
/// Whisper with `timestamp_granularities=["word"]`, Deepgram, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct TranscribedWord {
    /// Time interval covering this word.
    pub time_span: TimeSpan,
    /// The word as the backend transcribed it.
    pub text: String,
    /// Per-word confidence, if reported.
    pub confidence: Option<Confidence>,
}
