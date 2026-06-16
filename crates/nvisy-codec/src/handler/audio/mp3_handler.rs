//! MP3 handler: holds raw MP3 audio bytes and exposes them as a
//! single-track audio handle via [`Handler<Audio>`].
//!
//! [`Handler::redact`] decodes the stream via Symphonia, mutates an
//! interleaved `f32` PCM buffer with the same
//! [`super::redact::apply`] helper the WAV handler uses, then
//! re-encodes via `mp3lame-encoder` at the input clip's measured
//! average bitrate. The round-trip is therefore lossy in addition to
//! whatever loss the input already had — operators wanting bit-exact
//! preservation of unredacted regions should not redact MP3 inputs.
//!
//! The MP3 encoder dependency carries an LGPL-3.0 licence (via
//! libmp3lame) and requires a C toolchain plus `autoconf` /
//! `automake` at build time. Both prerequisites travel with the
//! `mp3` crate feature.
//!
//! [`Handler<Audio>`]: crate::Handler
//! [`Handler::redact`]: crate::Handler::redact
//! [`super::redact::apply`]: super::redact::apply

use bytes::Bytes;
use nvisy_core::Error;
use nvisy_core::modality::{Audio, AudioData, AudioLocation};
use nvisy_core::primitive::TimeSpan;
use nvisy_core::redaction::Redactions;

use super::duration::probe_duration_us;
use super::mp3_codec::{decode_to_pcm, encode_from_pcm};
use super::{Mp3Loader, redact};
use crate::content::{ContentData, ContentSource};
use crate::{Chunk, Format, FormatId, Handler};

/// Stable [`FormatId`] for the MP3 codec.
pub const FORMAT_ID: FormatId = FormatId::from_static("nvisy.audio.mp3");

/// [`Format`] descriptor registered into [`crate::CodecRegistry`].
pub fn format() -> Format {
    Format::new::<Audio, _>(FORMAT_ID.clone(), Mp3Loader)
        .with_extensions(["mp3"])
        .with_content_types(["audio/mpeg"])
}

/// Handler for loaded MP3 content.
#[derive(Debug)]
pub struct Mp3Handler {
    source: ContentSource,
    bytes: Bytes,
    filename: String,
    yielded: bool,
}

impl Mp3Handler {
    /// Create a handler from raw MP3 bytes.
    pub fn new(bytes: Bytes) -> Self {
        Self {
            source: ContentSource::new(),
            bytes,
            filename: "audio.mp3".to_owned(),
            yielded: false,
        }
    }

    /// Attach a content source for lineage tracking.
    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }

    /// Attach a filename hint for downstream extractors.
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = filename.into();
        self
    }

    /// Reference to the raw audio bytes.
    pub fn bytes(&self) -> &Bytes {
        &self.bytes
    }

    /// Rewind the streaming cursor.
    pub fn rewind(&mut self) {
        self.yielded = false;
    }
}

#[async_trait::async_trait]
impl Handler<Audio> for Mp3Handler {
    fn format(&self) -> FormatId {
        FORMAT_ID.clone()
    }

    fn source(&self) -> ContentSource {
        self.source
    }

    #[tracing::instrument(name = "mp3.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<ContentData, Error> {
        tracing::Span::current().record("output_bytes", self.bytes.len());
        let source = ContentSource::new().with_parent(&self.source);
        Ok(ContentData::new(source, self.bytes.clone()))
    }

    async fn next_chunk(&mut self) -> Result<Option<Chunk<Audio>>, Error> {
        if self.yielded {
            return Ok(None);
        }
        let duration_us = probe_duration_us(&self.bytes, "mp3")?;
        let location = AudioLocation::new(TimeSpan::new(0, duration_us));
        let data = AudioData::new(self.bytes.clone()).with_filename(self.filename.clone());
        self.yielded = true;
        Ok(Some(Chunk { location, data, hints: Vec::new() }))
    }

    async fn read(&self, _location: &AudioLocation) -> Result<Option<AudioData>, Error> {
        Ok(Some(
            AudioData::new(self.bytes.clone()).with_filename(self.filename.clone()),
        ))
    }

    /// Apply spans right-to-left so a [`AudioReplacement::Remove`]
    /// doesn't invalidate earlier sample indices. The MP3 round-trip
    /// decodes via `symphonia`, mutates an interleaved `f32` PCM
    /// buffer, then re-encodes via `mp3lame-encoder` at the input's
    /// average bitrate so the output's size stays in the same
    /// ballpark.
    ///
    /// [`AudioReplacement::Remove`]: nvisy_core::redaction::AudioReplacement::Remove
    async fn redact(&mut self, mut redactions: Redactions<Audio>) -> Result<(), Error> {
        if redactions.is_empty() {
            return Ok(());
        }
        let original_bytes_len = self.bytes.len();
        let original_duration_us = probe_duration_us(&self.bytes, "mp3")?;

        let mut decoded = decode_to_pcm(&self.bytes)?;

        redactions.sort_descending();
        for (location, replacement) in redactions.into_items() {
            redact::apply(
                &mut decoded.samples,
                location.time_span,
                &replacement,
                decoded.sample_rate,
                decoded.channels,
            );
        }

        let target_bitrate_bps = average_bitrate_bps(original_bytes_len, original_duration_us);
        let new_bytes = encode_from_pcm(
            &decoded.samples,
            decoded.sample_rate,
            decoded.channels,
            target_bitrate_bps,
        )?;
        self.bytes = Bytes::from(new_bytes);
        Ok(())
    }
}

/// Compute the average bitrate of the source file in bits/sec, so
/// the re-encode lands roughly on the same target. Falls back to
/// 128 kbps when the duration is non-positive (degenerate input).
///
/// `file_bytes` is the **whole** MP3 file size including ID3v1/v2
/// tags and any embedded album art; this slightly biases the
/// computed bitrate above the true audio-payload bitrate. The
/// inflation is typically <5% (a few KB of tags against megabytes
/// of audio) and gets snapped to a discrete LAME variant anyway,
/// so the practical effect on output size is negligible. Worth
/// revisiting only if we ever ship clips with very large embedded
/// art relative to audio length.
fn average_bitrate_bps(file_bytes: usize, duration_us: i64) -> u32 {
    if duration_us <= 0 {
        return 128_000;
    }
    // bps = file_bits * 1_000_000 / duration_us
    let bits = (file_bytes as u128).saturating_mul(8);
    let bps = bits
        .saturating_mul(1_000_000)
        .checked_div(duration_us as u128)
        .unwrap_or(128_000);
    u32::try_from(bps).unwrap_or(320_000)
}

#[cfg(test)]
mod tests {
    use nvisy_core::redaction::AudioReplacement;

    use super::*;

    /// Mint a small mono MP3 fixture for round-trip tests: 1 second
    /// of constant-amplitude tone at 16 kHz, encoded at 64 kbps.
    fn fixture_mono_tone_mp3(seconds: usize) -> Bytes {
        let total = 16_000 * seconds;
        let samples: Vec<f32> = (0..total).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
        let encoded = encode_from_pcm(&samples, 16_000, 1, 64_000).expect("encode fixture");
        Bytes::from(encoded)
    }

    #[tokio::test]
    async fn empty_redactions_is_noop() {
        let bytes = fixture_mono_tone_mp3(1);
        let original = bytes.clone();
        let mut handler = Mp3Handler::new(bytes);
        let rs: Redactions<Audio> = Redactions::default();
        handler.redact(rs).await.unwrap();
        assert_eq!(handler.bytes(), &original);
    }

    #[tokio::test]
    async fn silence_redaction_round_trips_and_zeros_target_window() {
        let bytes = fixture_mono_tone_mp3(2);
        let mut handler = Mp3Handler::new(bytes);

        let mut rs = Redactions::new();
        rs.push(
            AudioLocation::new(TimeSpan::new(500_000, 1_500_000)),
            AudioReplacement::Silence,
        );
        handler.redact(rs).await.unwrap();

        // Decode the redacted MP3 back and assert the middle second is
        // (approximately) silence. We allow generous tolerance because
        // MP3 is lossy and the encoder smears boundaries slightly.
        let decoded = decode_to_pcm(handler.bytes()).unwrap();
        let sr = decoded.sample_rate as usize;
        // Sample a window inside the silenced region, away from the
        // boundary where smear is biggest.
        let start = sr * 3 / 4;
        let end = sr * 5 / 4;
        let mean_abs: f32 = decoded.samples[start..end]
            .iter()
            .map(|s| s.abs())
            .sum::<f32>()
            / (end - start) as f32;
        assert!(
            mean_abs < 0.05,
            "silenced region should be near zero, mean |s| = {mean_abs}"
        );
    }

    #[tokio::test]
    async fn remove_redaction_shortens_clip_duration() {
        let bytes = fixture_mono_tone_mp3(3);
        let original_duration = probe_duration_us(&bytes, "mp3").unwrap();
        let mut handler = Mp3Handler::new(bytes);

        let mut rs = Redactions::new();
        rs.push(
            AudioLocation::new(TimeSpan::new(1_000_000, 2_000_000)),
            AudioReplacement::Remove,
        );
        handler.redact(rs).await.unwrap();

        let new_duration = probe_duration_us(handler.bytes(), "mp3").unwrap();
        // Removed 1s out of 3s; LAME pads the edges, so expect roughly
        // 2s ± 200ms.
        let diff = (new_duration - 2_000_000).abs();
        assert!(
            diff < 200_000,
            "expected ~2s after 1s removal, got {new_duration} us"
        );
        assert!(
            new_duration < original_duration,
            "redacted duration must be shorter than original"
        );
    }

    #[tokio::test]
    async fn next_chunk_propagates_probe_error_for_garbage_bytes() {
        // Real MP3 fixtures are constructed inline above; here we only
        // need to confirm the handler wires the probe in and surfaces
        // failures rather than silently stamping (0, 0).
        let mut handler = Mp3Handler::new(Bytes::from_static(b"definitely not an mp3"));
        let err = handler.next_chunk().await.unwrap_err();
        assert!(err.to_string().contains("audio probe failed"));
    }
}
