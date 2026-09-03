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
    use gladia::model::TranscriptionLanguageCodeEnum;

    use super::*;

    /// An utterance built directly rather than parsed from JSON: these tests
    /// cover this crate's mapping, not the SDK's deserialization.
    fn utterance(start: f64, end: f64, words: Vec<WordDto>) -> UtteranceDto {
        UtteranceDto {
            channel: 0,
            confidence: 0.9,
            start,
            end,
            language: TranscriptionLanguageCodeEnum::En,
            speaker: None,
            text: "x".to_owned(),
            words,
        }
    }

    fn word(start: f64, end: f64) -> WordDto {
        WordDto {
            confidence: 0.9,
            start,
            end,
            word: "w".to_owned(),
        }
    }

    /// Rounding, not truncation: a word ending at 1.9996s must report 2000ms
    /// rather than gapping against a neighbour that starts at 2000ms.
    #[test]
    fn seconds_round_to_milliseconds() {
        assert_eq!(to_ms(1.9996), 2000);
        assert_eq!(to_ms(2.4), 2400);
    }

    /// A sub-millisecond inversion is still an inversion.
    ///
    /// Regression: checking after rounding accepted 1.0004s -> 1.0003s as a
    /// zero-length span, because both values round to 1000ms. The guard has
    /// to compare the seconds Gladia actually sent.
    #[test]
    fn drops_spans_inverted_below_millisecond_resolution() {
        assert!(segment(utterance(1.0004, 1.0003, vec![])).is_none());
        assert_eq!(
            segment(utterance(
                0.0,
                2.0,
                vec![word(1.0004, 1.0003), word(0.0, 0.5)]
            ))
            .unwrap()
            .words
            .len(),
            1
        );
    }

    /// Gladia's integer speaker index becomes the `SPEAKER_NN` label the
    /// other backends emit, so consumers see one vocabulary.
    #[test]
    fn speaker_index_becomes_a_label() {
        let mut diarized = utterance(0.0, 1.0, vec![]);
        diarized.speaker = Some(0);
        assert_eq!(
            segment(diarized).unwrap().speaker_id.as_deref(),
            Some("SPEAKER_00")
        );
    }
}
