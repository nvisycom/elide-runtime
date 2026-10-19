//! Translation from Deepgram's batch response into elide's audio
//! vocabulary.
//!
//! Deepgram reports **float seconds**; elide's [`TimeSpan`] is built from
//! whole milliseconds. [`to_ms`] is the single conversion point.
//!
//! Speaker indices become `SPEAKER_00`-style labels, matching what
//! pyannote and the self-hosted backend emit, so a consumer sees one
//! vocabulary regardless of which backend produced the transcript.

use deepgram::common::batch_response::{Response, Utterance, Word};
use elide_audio::modality::{TranscriptSegment, TranscriptWord};
use elide_audio::primitive::TimeSpan;
use elide_audio::stt::SttResponse;
use elide_core::primitive::Confidence;

use crate::error::DeepgramBackendError;

/// Float seconds to whole milliseconds.
///
/// Rounds rather than truncates, so a word ending at 1.9996s reports
/// 2000ms instead of gapping against a neighbour starting at 2000ms.
/// Negative and non-finite values clamp to zero rather than wrapping on
/// the cast.
pub(crate) fn to_ms(seconds: f64) -> u64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0;
    }
    (seconds * 1000.0).round() as u64
}

/// Deepgram's integer speaker index as the `SPEAKER_NN` label elide
/// consumers see from every other diarizing backend.
fn speaker_label(index: usize) -> String {
    format!("SPEAKER_{index:02}")
}

/// The span for `start..end` in float seconds, or `None` when inverted.
///
/// The comparison is in seconds, before rounding: 1.0004s to 1.0003s is
/// inverted, but both round to 1000ms, so a millisecond comparison would
/// accept it as a zero-length span instead of rejecting it. An inverted
/// span means the provider disagreed with itself, and inventing one would
/// hide that from the caller.
fn span(start: f64, end: f64) -> Option<TimeSpan> {
    (end >= start).then(|| TimeSpan::from_millis(to_ms(start), to_ms(end)))
}

/// The transcript from a batch response.
///
/// Requires `utterances`: the alternative is a single flat transcript per
/// channel, which carries no segment boundaries. The backend always asks
/// for utterances, so their absence means the provider did not answer the
/// request that was made.
pub(crate) fn decode(response: Response) -> Result<SttResponse, DeepgramBackendError> {
    let utterances = response.results.utterances.ok_or_else(|| {
        DeepgramBackendError::Protocol(
            "response carried no utterances; the request asks for them".to_owned(),
        )
    })?;

    Ok(SttResponse::new(
        utterances.into_iter().filter_map(segment).collect(),
    ))
}

/// One segment, or `None` when its span is unusable.
///
/// An inverted span is dropped rather than clamped; see [`span`].
fn segment(utterance: Utterance) -> Option<TranscriptSegment> {
    let mut segment = TranscriptSegment::new(
        span(utterance.start, utterance.end)?,
        utterance.transcript.trim().to_owned(),
    );
    if let Some(index) = utterance.speaker {
        segment = segment.with_speaker_id(speaker_label(index));
    }
    segment = segment.with_confidence(Confidence::clamped(utterance.confidence as f32));

    let words: Vec<_> = utterance.words.into_iter().filter_map(word).collect();
    if !words.is_empty() {
        segment = segment.with_words(words);
    }
    Some(segment)
}

/// One word, or `None` when its span or text is unusable.
///
/// Deepgram also reports a per-word `speaker` when diarization is on, but
/// elide models speaker attribution on [`TranscriptSegment`] alone —
/// [`TranscriptWord`] has no such field — so it is dropped here. A word
/// disagreeing with its own utterance's speaker is not representable.
fn word(dto: Word) -> Option<TranscriptWord> {
    let text = dto.word.trim();
    if text.is_empty() {
        return None;
    }
    Some(
        TranscriptWord::new(span(dto.start, dto.end)?, text.to_owned())
            .with_confidence(Confidence::clamped(dto.confidence as f32)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rounding, not truncation: a word ending at 1.9996s must report
    /// 2000ms rather than gapping against a neighbour starting at 2000ms.
    #[test]
    fn seconds_round_to_milliseconds() {
        assert_eq!(to_ms(1.9996), 2000);
        assert_eq!(to_ms(2.4), 2400);
    }

    /// A negative or non-finite value clamps rather than wrapping on the
    /// cast to `u64`.
    #[test]
    fn out_of_range_seconds_clamp_to_zero() {
        assert_eq!(to_ms(-0.5), 0);
        assert_eq!(to_ms(f64::NAN), 0);
        assert_eq!(to_ms(f64::INFINITY), 0);
    }

    /// A sub-millisecond inversion is still an inversion.
    ///
    /// Checking after rounding would accept 1.0004s -> 1.0003s as a
    /// zero-length span, because both values round to 1000ms. This is the
    /// bug a review caught in the Gladia backend.
    #[test]
    fn rejects_spans_inverted_below_millisecond_resolution() {
        assert!(span(1.0004, 1.0003).is_none());
        assert!(span(2.0, 1.0).is_none());
        // Zero-length is legal: a genuine point in time.
        assert!(span(1.0, 1.0).is_some());
    }

    /// Deepgram's integer speaker index becomes the `SPEAKER_NN` label the
    /// other backends emit, so consumers see one vocabulary.
    #[test]
    fn speaker_index_becomes_a_label() {
        assert_eq!(speaker_label(0), "SPEAKER_00");
        assert_eq!(speaker_label(12), "SPEAKER_12");
    }
}
