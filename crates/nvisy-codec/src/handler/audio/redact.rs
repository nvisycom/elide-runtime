//! Per-sample-buffer audio redaction helper shared by every audio
//! handler (WAV today; MP3 doesn't redact and so doesn't use it).
//!
//! Applies one [`AudioReplacement`] to a flat, channel-interleaved
//! `Vec<S>` sample buffer in place, given the replacement's containing
//! time span and the buffer's sample rate + channel count.
//!
//! Ordering across multiple replacements is the caller's
//! responsibility: an [`AudioReplacement::Remove`] shrinks the buffer,
//! so later time spans must be applied first to keep earlier ones'
//! indices valid. Audio handlers typically override
//! [`Handler::redact`] to use [`sort_redactions_for_audio`].
//!
//! [`Handler::redact`]: crate::Handler::redact
//! [`sort_redactions_for_audio`]: crate::handler::audio::sort_redactions_for_audio

use nvisy_core::primitive::TimeSpan;
use nvisy_core::redaction::AudioReplacement;

const TARGET: &str = "nvisy_codec::handler::audio::redact";

/// Apply a single replacement to `samples` in place.
pub(crate) fn apply<S>(
    samples: &mut Vec<S>,
    time_span: TimeSpan,
    replacement: &AudioReplacement,
    sample_rate: u32,
    channels: u16,
) where
    S: Default + Clone,
{
    let (start_sample, end_sample) = time_span.sample_range(sample_rate, channels);
    let start = start_sample.min(samples.len());
    let end = end_sample.min(samples.len());
    if start >= end {
        return;
    }
    match replacement {
        AudioReplacement::Silence => {
            for s in &mut samples[start..end] {
                *s = S::default();
            }
        }
        AudioReplacement::Remove => {
            samples.drain(start..end);
        }
        AudioReplacement::Replace { .. } => {
            tracing::warn!(
                target: TARGET,
                start_us = time_span.start_us,
                end_us = time_span.end_us,
                "AudioReplacement::Replace is not yet implemented, skipping",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    #[test]
    fn silence_zeroes_range_mono() {
        let mut samples: Vec<i16> = (1..=10).collect();
        apply(
            &mut samples,
            TimeSpan::new(3_000, 6_000),
            &AudioReplacement::Silence,
            1000,
            1,
        );
        assert_eq!(samples, vec![1, 2, 3, 0, 0, 0, 7, 8, 9, 10]);
    }

    #[test]
    fn remove_shrinks_range_mono() {
        let mut samples: Vec<i16> = (1..=10).collect();
        apply(
            &mut samples,
            TimeSpan::new(3_000, 6_000),
            &AudioReplacement::Remove,
            1000,
            1,
        );
        assert_eq!(samples, vec![1, 2, 3, 7, 8, 9, 10]);
    }

    #[test]
    fn stereo_silence_aligns_to_frames() {
        let mut samples: Vec<i16> = (1..=20).collect();
        apply(
            &mut samples,
            TimeSpan::new(3_000, 6_000),
            &AudioReplacement::Silence,
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
        apply(
            &mut samples,
            TimeSpan::new(3_000, 6_000),
            &AudioReplacement::Remove,
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
        apply(
            &mut samples,
            TimeSpan::new(0, 999_999_000),
            &AudioReplacement::Silence,
            1000,
            1,
        );
        assert_eq!(samples, vec![0, 0, 0, 0, 0]);
    }

    #[test]
    fn replace_is_warned_and_skipped() {
        let mut samples: Vec<i16> = (1..=5).collect();
        apply(
            &mut samples,
            TimeSpan::new(0, 3_000),
            &AudioReplacement::Replace { data: Bytes::new() },
            1000,
            1,
        );
        assert_eq!(samples, vec![1, 2, 3, 4, 5]);
    }
}
