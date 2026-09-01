//! Translation from Gladia's transcription result into elide's audio
//! vocabulary.
//!
//! Two shape differences are handled here:
//!
//! - Gladia reports **float seconds**; elide's [`TimeSpan`] is built from
//!   whole milliseconds. [`to_ms`] is the single conversion point.
//! - Gladia's `speaker` is an **integer** index; elide's `speaker_id` is an
//!   opaque string label. We format it as `SPEAKER_00` to match what
//!   pyannote and WhisperX emit, so a consumer sees one vocabulary
//!   regardless of which backend produced the transcript.

use elide_core::modality::audio::{TranscriptSegment, TranscriptWord};
use elide_core::primitive::{Confidence, LanguageTag, TimeSpan};
use elide_stt::SttResponse;
use gladia::model::{PreRecordedResponse, PreRecordedResponseStatus, UtteranceDto, WordDto};

use crate::error::GladiaError;

/// Float seconds to whole milliseconds.
///
/// Rounds rather than truncates, so a word ending at 1.9996s reports 2000ms
/// instead of gapping against a neighbour starting at 2000ms. Negative and
/// non-finite values clamp to zero rather than wrapping on the cast.
pub(crate) fn to_ms(seconds: f64) -> u64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0;
    }
    (seconds * 1000.0).round() as u64
}

/// Gladia's integer speaker index as the `SPEAKER_NN` label elide
/// consumers see from every other diarizing backend.
fn speaker_label(index: u64) -> String {
    format!("SPEAKER_{index:02}")
}

/// The transcript for a finished job.
///
/// A job in a non-`done` state is an error rather than an empty transcript,
/// and so is a `done` job with no result body — Gladia's schema makes the
/// result present on success, so its absence is the provider contradicting
/// itself, not a silent empty answer.
pub(crate) fn decode(response: PreRecordedResponse) -> Result<SttResponse, GladiaError> {
    if response.status != PreRecordedResponseStatus::Done {
        return Err(GladiaError::Failed(format!(
            "job finished as {:?} (error_code {:?})",
            response.status, response.error_code
        )));
    }
    let utterances = response
        .result
        .and_then(|result| result.transcription)
        .map(|transcription| transcription.utterances)
        .ok_or_else(|| GladiaError::Failed("done job carried no transcription".to_owned()))?;

    Ok(SttResponse::new(
        utterances.into_iter().filter_map(segment).collect(),
    ))
}

/// One segment, or `None` when its span is unusable.
///
/// An inverted span is dropped rather than clamped: it means the provider
/// disagreed with itself, and inventing a span would hide that.
fn segment(utterance: UtteranceDto) -> Option<TranscriptSegment> {
    // Checked in seconds, before rounding: 1.0004s -> 1.0003s is inverted,
    // but both round to 1000ms, so a millisecond comparison would accept it
    // as a zero-length span rather than dropping it.
    if utterance.end < utterance.start {
        return None;
    }
    let (start_ms, end_ms) = (to_ms(utterance.start), to_ms(utterance.end));

    let mut segment = TranscriptSegment::new(
        TimeSpan::from_millis(start_ms, end_ms),
        utterance.text.trim().to_owned(),
    );
    if let Some(index) = utterance.speaker {
        segment = segment.with_speaker_id(speaker_label(index));
    }
    if let Ok(tag) = utterance.language.to_string().parse::<LanguageTag>() {
        segment = segment.with_language(tag);
    }
    segment = segment.with_confidence(Confidence::clamped(utterance.confidence as f32));

    let words: Vec<_> = utterance.words.into_iter().filter_map(word).collect();
    if !words.is_empty() {
        segment = segment.with_words(words);
    }
    Some(segment)
}

/// One word, or `None` when its span or text is unusable.
fn word(dto: WordDto) -> Option<TranscriptWord> {
    // Inversion is checked in seconds, before rounding, for the same reason
    // as in `segment`.
    let text = dto.word.trim();
    if dto.end < dto.start || text.is_empty() {
        return None;
    }
    let (start_ms, end_ms) = (to_ms(dto.start), to_ms(dto.end));
    Some(
        TranscriptWord::new(TimeSpan::from_millis(start_ms, end_ms), text.to_owned())
            .with_confidence(Confidence::clamped(dto.confidence as f32)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a `PreRecordedResponse` from a JSON body, so the tests exercise
    /// the same deserialization path a live call takes.
    fn response(json: &str) -> PreRecordedResponse {
        serde_json::from_str(json).expect("fixture should match the SDK model")
    }

    fn done_body(utterances: &str) -> String {
        format!(
            r#"{{
              "id": "0199a1a0-0000-7000-8000-000000000000",
              "request_id": "req", "version": 2, "status": "done",
              "created_at": "2026-09-01T10:00:00Z",
              "kind": "pre-recorded", "post_session_metadata": {{}},
              "result": {{ "metadata": {{
                    "audio_duration": 2.4, "number_of_distinct_channels": 1,
                    "billing_time": 2.4, "transcription_time": 1.0 }},
                 "transcription": {{
                    "full_transcript": "Hello there.", "languages": ["en"],
                    "utterances": [{utterances}] }} }}
            }}"#
        )
    }

    /// Float seconds round to the nearest millisecond rather than truncating,
    /// so a word ending at 1.9996s does not gap against one starting at 2.0s.
    #[test]
    fn seconds_round_to_milliseconds() {
        assert_eq!(to_ms(1.9996), 2000);
        assert_eq!(to_ms(0.0), 0);
        assert_eq!(to_ms(2.4), 2400);
    }

    /// A negative or non-finite span clamps rather than wrapping on the cast.
    #[test]
    fn out_of_range_seconds_clamp_to_zero() {
        assert_eq!(to_ms(-0.5), 0);
        assert_eq!(to_ms(f64::NAN), 0);
        assert_eq!(to_ms(f64::INFINITY), 0);
    }

    /// A finished job decodes into segments, with the integer speaker index
    /// rendered as the `SPEAKER_NN` label the other backends emit.
    #[test]
    fn decodes_done_job_with_diarization() {
        let body = done_body(
            r#"{ "start": 0.0, "end": 2.4, "text": " Hello there. ",
                 "confidence": 0.94, "channel": 0, "speaker": 0, "language": "en",
                 "words": [{ "word": "Hello", "start": 0.0, "end": 0.48,
                             "confidence": 0.97 }] }"#,
        );
        let decoded = decode(response(&body)).unwrap();
        assert_eq!(decoded.segments.len(), 1);

        let segment = &decoded.segments[0];
        assert_eq!(segment.span.start_millis(), 0);
        assert_eq!(segment.span.end_millis(), 2400);
        // Text is trimmed: Gladia pads utterances with a leading space.
        assert_eq!(segment.text, "Hello there.");
        assert_eq!(segment.speaker_id.as_deref(), Some("SPEAKER_00"));
        assert_eq!(segment.words.len(), 1);
        assert_eq!(segment.words[0].span.end_millis(), 480);
    }

    /// Without diarization there is no speaker, and the rest still decodes.
    #[test]
    fn decodes_without_speaker() {
        let body = done_body(
            r#"{ "start": 0.0, "end": 1.0, "text": "x", "confidence": 0.5,
                 "channel": 0, "language": "en", "words": [] }"#,
        );
        let decoded = decode(response(&body)).unwrap();
        assert!(decoded.segments[0].speaker_id.is_none());
        assert!(decoded.segments[0].words.is_empty());
    }

    /// An inverted span means the provider disagreed with itself; the
    /// utterance is dropped rather than clamped into a plausible-looking one.
    #[test]
    fn drops_inverted_spans() {
        let body = done_body(
            r#"{ "start": 2.0, "end": 1.0, "text": "inverted", "confidence": 0.9,
                 "channel": 0, "language": "en", "words": [] },
               { "start": 0.0, "end": 1.0, "text": "good", "confidence": 0.9,
                 "channel": 0, "language": "en", "words": [] }"#,
        );
        let decoded = decode(response(&body)).unwrap();
        assert_eq!(decoded.segments.len(), 1);
        assert_eq!(decoded.segments[0].text, "good");
    }

    /// A sub-millisecond inversion is still an inversion.
    ///
    /// Regression: checking after rounding accepted 1.0004s -> 1.0003s as a
    /// zero-length span, because both values round to 1000ms.
    #[test]
    fn drops_spans_inverted_below_millisecond_resolution() {
        let body = done_body(
            r#"{ "start": 1.0004, "end": 1.0003, "text": "inverted", "confidence": 0.9,
                 "channel": 0, "language": "en",
                 "words": [{ "word": "x", "start": 2.0004, "end": 2.0003,
                             "confidence": 0.9 }] }"#,
        );
        assert!(decode(response(&body)).unwrap().segments.is_empty());
    }

    /// A word inverted below millisecond resolution is dropped, while its
    /// segment survives.
    #[test]
    fn drops_words_inverted_below_millisecond_resolution() {
        let body = done_body(
            r#"{ "start": 0.0, "end": 2.0, "text": "x", "confidence": 0.9,
                 "channel": 0, "language": "en",
                 "words": [{ "word": "bad", "start": 1.0004, "end": 1.0003,
                             "confidence": 0.9 },
                           { "word": "good", "start": 0.0, "end": 0.5,
                             "confidence": 0.9 }] }"#,
        );
        let decoded = decode(response(&body)).unwrap();
        let words = &decoded.segments[0].words;
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "good");
    }

    /// A failed job surfaces as an error, not as an empty transcript.
    #[test]
    fn failed_job_is_an_error() {
        let body = r#"{
            "id": "0199a1a0-0000-7000-8000-000000000000",
            "request_id": "req", "version": 2, "status": "error",
            "created_at": "2026-09-01T10:00:00Z", "error_code": 500,
            "kind": "pre-recorded", "post_session_metadata": {}
        }"#;
        assert!(decode(response(body)).is_err());
    }

    /// A `done` job with no result body contradicts Gladia's own schema, so
    /// it is an error rather than a silent empty transcript.
    #[test]
    fn done_without_result_is_an_error() {
        let body = r#"{
            "id": "0199a1a0-0000-7000-8000-000000000000",
            "request_id": "req", "version": 2, "status": "done",
            "created_at": "2026-09-01T10:00:00Z",
            "kind": "pre-recorded", "post_session_metadata": {}
        }"#;
        assert!(decode(response(body)).is_err());
    }
}
