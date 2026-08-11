//! Incoming wire types for the STT `/transcribe` endpoint.
//!
//! Mirrors `nvisy_core.stt.v1.SttResponse` from the inference
//! repository: ordered `segments`, each with millisecond timings,
//! the recognised text, and optional diarization / language /
//! confidence / per-word breakdowns. Fields the service emits but
//! elide's [`TranscriptSegment`] does not model yet
//! (response-level `modelId`, per-segment `channel`) are
//! deserialised-and-discarded.
//!
//! [`TranscriptSegment`]: elide_core::modality::audio::TranscriptSegment

use elide_core::modality::audio::{TranscriptSegment, TranscriptWord};
use elide_core::primitive::{Confidence, LanguageTag, TimeSpan};
use elide_stt::SttResponse;
use serde::Deserialize;

/// Incoming per-call response body.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireSttResponse {
    #[serde(default)]
    pub segments: Vec<WireSegment>,
    // `modelId` ignored: provenance comes from `BentoStt::model_id`
    // (the deployment-level id the operator wired at construction).
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireSegment {
    /// Segment start in whole milliseconds from stream start.
    pub start_ms: u64,
    /// Segment end (exclusive) in whole milliseconds from stream start.
    pub end_ms: u64,
    /// Recognised text for the segment.
    pub text: String,
    /// Diarization speaker label, when the backend assigned one.
    #[serde(default)]
    pub speaker_id: Option<String>,
    /// Detected per-segment language as a BCP-47 tag, when reported.
    #[serde(default)]
    pub language: Option<String>,
    /// Segment-level confidence, when reported.
    #[serde(default)]
    pub confidence: Option<f32>,
    /// Word-level breakdown, when the backend emits one.
    #[serde(default)]
    pub words: Vec<WireWord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireWord {
    /// Word start in whole milliseconds from stream start.
    pub start_ms: u64,
    /// Word end (exclusive) in whole milliseconds from stream start.
    pub end_ms: u64,
    /// The word text, as it appears in the segment.
    pub text: String,
    /// Per-word confidence, when reported.
    #[serde(default)]
    pub confidence: Option<f32>,
}

impl WireSttResponse {
    /// Translate into the elide [`SttResponse`] the backend trait
    /// expects.
    pub(super) fn decode(self) -> SttResponse {
        SttResponse::new(self.segments.into_iter().map(WireSegment::decode).collect())
    }
}

impl WireSegment {
    fn decode(self) -> TranscriptSegment {
        let span = TimeSpan::from_millis(self.start_ms, self.end_ms);
        let mut segment = TranscriptSegment::new(span, self.text);
        if let Some(id) = self.speaker_id {
            segment = segment.with_speaker_id(id);
        }
        if let Some(tag) = self.language.and_then(|s| s.parse::<LanguageTag>().ok()) {
            segment = segment.with_language(tag);
        }
        if let Some(c) = self.confidence {
            segment = segment.with_confidence(Confidence::clamped(c));
        }
        if !self.words.is_empty() {
            segment = segment.with_words(self.words.into_iter().map(WireWord::decode).collect());
        }
        segment
    }
}

impl WireWord {
    fn decode(self) -> TranscriptWord {
        let span = TimeSpan::from_millis(self.start_ms, self.end_ms);
        let mut word = TranscriptWord::new(span, self.text);
        if let Some(c) = self.confidence {
            word = word.with_confidence(Confidence::clamped(c));
        }
        word
    }
}
