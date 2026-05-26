//! Helper for applying a single [`AudioRedaction`] to a typed sample
//! buffer in place.

use nvisy_ontology::primitive::TimeSpan;

use crate::handler::{AudioOutput, AudioRedaction};

const TARGET: &str = "nvisy_codec::handler::audio";

/// Apply a single redaction to `samples` in place.
///
/// `samples` is a flat, channel-interleaved buffer of `S`. `channels`
/// is the number of channels (1 for mono, 2 for stereo). `sample_rate`
/// is the sample rate in Hz. The redaction expresses its range as a
/// [`TimeSpan`] supplied separately by the caller — under the
/// `(location, redaction)` shape the time span lives on the
/// [`AudioLocation`], not the redaction.
///
/// Ordering across multiple redactions is the caller's
/// responsibility: an [`AudioOutput::Remove`] shrinks the buffer, so
/// later time spans must be applied first to keep earlier ones'
/// indices valid. See [`AudioHandler::redact`].
///
/// [`AudioLocation`]: nvisy_ontology::entity::AudioLocation
/// [`AudioHandler::redact`]: crate::handler::AudioHandler::redact
pub fn apply_audio_redaction<S>(
    samples: &mut Vec<S>,
    time_span: TimeSpan,
    redaction: &AudioRedaction,
    sample_rate: u32,
    channels: u16,
) where
    S: Default + Clone,
{
    let (start_sample, end_sample) =
        samples_for_time_span(time_span.start_us, time_span.end_us, sample_rate, channels);
    let start = start_sample.min(samples.len());
    let end = end_sample.min(samples.len());
    if start >= end {
        return;
    }
    match &redaction.output {
        AudioOutput::Silence => {
            for s in &mut samples[start..end] {
                *s = S::default();
            }
        }
        AudioOutput::Remove => {
            samples.drain(start..end);
        }
        AudioOutput::Replace { .. } => {
            tracing::warn!(
                target: TARGET,
                start_us = time_span.start_us,
                end_us = time_span.end_us,
                "AudioOutput::Replace is not yet implemented, skipping",
            );
        }
    }
}

/// Convert a `[start_us, end_us)` time span to a `[start_sample,
/// end_sample)` index range into a channel-interleaved sample buffer.
///
/// Rounds half-up at the frame boundary, then multiplies by `channels`
/// so the returned indices land on frame boundaries (no stereo channel
/// swap on [`AudioOutput::Remove`]).
fn samples_for_time_span(
    start_us: i64,
    end_us: i64,
    sample_rate: u32,
    channels: u16,
) -> (usize, usize) {
    let start_frame = us_to_frame(start_us, sample_rate);
    let end_frame = us_to_frame(end_us, sample_rate);
    (
        start_frame.saturating_mul(channels as usize),
        end_frame.saturating_mul(channels as usize),
    )
}

fn us_to_frame(us: i64, sample_rate: u32) -> usize {
    if us <= 0 {
        return 0;
    }
    let num = (us as u128) * (sample_rate as u128) + 500_000;
    (num / 1_000_000) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_zeroes_range_mono() {
        let mut samples: Vec<i16> = (1..=10).collect();
        apply_audio_redaction(
            &mut samples,
            TimeSpan::new(3_000, 6_000),
            &AudioRedaction::new(AudioOutput::Silence),
            1000,
            1,
        );
        assert_eq!(samples, vec![1, 2, 3, 0, 0, 0, 7, 8, 9, 10]);
    }

    #[test]
    fn remove_shrinks_range_mono() {
        let mut samples: Vec<i16> = (1..=10).collect();
        apply_audio_redaction(
            &mut samples,
            TimeSpan::new(3_000, 6_000),
            &AudioRedaction::new(AudioOutput::Remove),
            1000,
            1,
        );
        assert_eq!(samples, vec![1, 2, 3, 7, 8, 9, 10]);
    }

    #[test]
    fn stereo_silence_aligns_to_frames() {
        let mut samples: Vec<i16> = (1..=20).collect();
        apply_audio_redaction(
            &mut samples,
            TimeSpan::new(3_000, 6_000),
            &AudioRedaction::new(AudioOutput::Silence),
            1000,
            2,
        );
        assert_eq!(
            samples,
            vec![
                1, 2, 3, 4, 5, 6, 0, 0, 0, 0, 0, 0, 13, 14, 15, 16, 17, 18, 19, 20
            ],
        );
    }

    #[test]
    fn stereo_remove_drops_frames_not_samples() {
        let mut samples: Vec<i16> = (1..=20).collect();
        apply_audio_redaction(
            &mut samples,
            TimeSpan::new(3_000, 6_000),
            &AudioRedaction::new(AudioOutput::Remove),
            1000,
            2,
        );
        assert_eq!(samples.len(), 14);
        assert_eq!(
            samples,
            vec![1, 2, 3, 4, 5, 6, 13, 14, 15, 16, 17, 18, 19, 20]
        );
    }

    #[test]
    fn out_of_bounds_clipped() {
        let mut samples: Vec<i16> = (1..=5).collect();
        apply_audio_redaction(
            &mut samples,
            TimeSpan::new(0, 999_999_000),
            &AudioRedaction::new(AudioOutput::Silence),
            1000,
            1,
        );
        assert_eq!(samples, vec![0, 0, 0, 0, 0]);
    }

    #[test]
    fn replace_is_warned_and_skipped() {
        let mut samples: Vec<i16> = (1..=5).collect();
        apply_audio_redaction(
            &mut samples,
            TimeSpan::new(0, 3_000),
            &AudioRedaction::new(AudioOutput::Replace { data: vec![] }),
            1000,
            1,
        );
        assert_eq!(samples, vec![1, 2, 3, 4, 5]);
    }
}
