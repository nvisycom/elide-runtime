//! Incoming wire types for the STT `/transcribe` endpoint.
//!
//! Generated from `elide_core.stt.v1.SttResponse`: ordered `segments`, each with millisecond timings,
//! the recognised text, and optional diarization / language /
//! confidence / per-word breakdowns. Fields the service emits but
//! elide's [`TranscriptSegment`] does not model yet
//! (response-level `model_id`, language, and duration) are
//! deserialised-and-discarded.
//!
//! [`TranscriptSegment`]: elide_audio::modality::TranscriptSegment

use elide_audio::modality::{TranscriptSegment, TranscriptWord};
use elide_audio::primitive::TimeSpan;
use elide_audio::stt::SttResponse;
use elide_core::primitive::{Confidence, LanguageTag};

use super::wire::{Segment as WireSegment, SttResponse as WireSttResponse, Word as WireWord};
use crate::error::BentoError;

impl WireSttResponse {
    /// Translate into the elide [`SttResponse`] the backend trait
    /// expects.
    pub(super) fn decode(self) -> Result<SttResponse, BentoError> {
        for segment in &self.segments {
            if segment.end_ms < segment.start_ms
                || segment.end_ms > u64::MAX / 1000
                || segment
                    .words
                    .iter()
                    .any(|word| word.end_ms < word.start_ms || word.end_ms > u64::MAX / 1000)
            {
                return Err(BentoError::Protocol(
                    "STT timing is inverted or exceeds the supported range".into(),
                ));
            }
        }
        Ok(SttResponse::new(
            self.segments.into_iter().map(WireSegment::decode).collect(),
        ))
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
            segment = segment.with_confidence(Confidence::clamped(c as f32));
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
            word = word.with_confidence(Confidence::clamped(c as f32));
        }
        word
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_fixtures_preserve_milliseconds_speakers_and_words() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../contracts/stt/v1/fixtures.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let wire: WireSttResponse = serde_json::from_value(case["response"].clone()).unwrap();
            let segments = wire.decode().unwrap().segments;
            let expected = case["expected_segments"].as_array().unwrap();
            assert_eq!(segments.len(), expected.len());
            for (segment, expected) in segments.iter().zip(expected) {
                assert_eq!(
                    segment.span.start_millis(),
                    expected["start_ms"].as_u64().unwrap()
                );
                assert_eq!(
                    segment.span.end_millis(),
                    expected["end_ms"].as_u64().unwrap()
                );
                assert_eq!(segment.text, expected["text"].as_str().unwrap());
                assert_eq!(
                    segment.speaker_id.as_ref().map(|id| id.as_str()),
                    expected["speaker_id"].as_str()
                );
                assert_eq!(
                    segment.language.as_ref().map(ToString::to_string),
                    expected["language"].as_str().map(str::to_owned)
                );
                assert_eq!(
                    segment.confidence,
                    expected["confidence"]
                        .as_f64()
                        .map(|c| Confidence::clamped(c as f32))
                );
                let words = expected["words"].as_array().unwrap();
                assert_eq!(segment.words.len(), words.len());
                for (word, expected) in segment.words.iter().zip(words) {
                    assert_eq!(
                        word.span.start_millis(),
                        expected["start_ms"].as_u64().unwrap()
                    );
                    assert_eq!(word.span.end_millis(), expected["end_ms"].as_u64().unwrap());
                    assert_eq!(word.text, expected["text"].as_str().unwrap());
                    assert_eq!(
                        word.confidence,
                        expected["confidence"]
                            .as_f64()
                            .map(|c| Confidence::clamped(c as f32))
                    );
                }
            }
        }
    }

    #[test]
    fn optional_fields_can_be_omitted() {
        let wire: WireSttResponse = serde_json::from_str(
            r#"{"model_id":"m","segments":[{"start_ms":0,"end_ms":0,"text":""}]}"#,
        )
        .unwrap();
        let response = wire.decode().unwrap();
        assert!(response.segments[0].speaker_id.is_none());
        assert!(response.segments[0].words.is_empty());
    }

    #[test]
    fn inverted_and_unrepresentable_timings_are_rejected() {
        for (start, end) in [(10, 5), (0, u64::MAX)] {
            let wire: WireSttResponse = serde_json::from_value(serde_json::json!({
                "model_id":"m", "segments":[{"start_ms":start,"end_ms":end,"text":"a"}]
            }))
            .unwrap();
            assert!(wire.decode().is_err());
        }
    }
}
