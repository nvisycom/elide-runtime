//! MP3 decode + re-encode helpers for the redaction round-trip.
//!
//! Two helpers, each owning one side of the redact-via-PCM strategy
//! the MP3 handler uses:
//!
//! - [`decode_to_pcm`] turns MP3 bytes into a flat, channel-
//!   interleaved `Vec<f32>` plus the sample rate / channel count it
//!   was decoded at. Sample-rate and channel-count come from the
//!   first audio track's codec parameters and are assumed constant
//!   across the clip (LAME requires them at builder time).
//! - [`encode_from_pcm`] turns an interleaved `Vec<f32>` back into
//!   MP3 bytes via `mp3lame-encoder` at a caller-supplied bitrate.
//!
//! Re-encoding is lossy in addition to whatever loss the input
//! already had; the pair only exists to support sample-level
//! redaction of MP3 streams. Callers wanting bit-perfect preservation
//! of unredacted regions should not round-trip.

use bytes::Bytes;
use mp3lame_encoder::{Builder, FlushNoGap, InterleavedPcm, MonoPcm};
use nvisy_core::Error;
use symphonia::core::audio::{Audio as _, GenericAudioBufferRef};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::default::{get_codecs, get_probe};

use std::io::Cursor;

const TARGET: &str = "nvisy_codec::handler::audio::mp3_codec";

/// Decoded MP3 contents in interleaved `f32` PCM form.
pub(super) struct DecodedMp3 {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Probe `bytes` and return the channel count of the first audio
/// track. Cheap — runs only the format probe + codec-params lookup,
/// no packet decoding.
///
/// Used by the loader as a gate so >2-channel inputs are rejected
/// before they ever reach the redact path (LAME's encoder side
/// only supports mono and stereo, and silent downmixing would
/// quietly edit the *unredacted* audio).
pub(super) fn probe_channels(bytes: &Bytes) -> Result<u16, Error> {
    let mss = MediaSourceStream::new(
        Box::new(Cursor::new(bytes.clone())),
        Default::default(),
    );
    let mut hint = Hint::new();
    hint.with_extension("mp3");

    let reader = get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| Error::validation(format!("MP3 probe failed: {e}"), TARGET))?;

    let track = reader
        .default_track(TrackType::Audio)
        .ok_or_else(|| Error::validation("MP3 stream has no audio track", TARGET))?;

    let channels = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .and_then(|a| a.channels.as_ref())
        .map(|c| c.count())
        .ok_or_else(|| Error::validation("MP3 track is missing channel info", TARGET))?;

    u16::try_from(channels).map_err(|_| {
        Error::validation(
            format!("MP3 track channel count {channels} exceeds u16"),
            TARGET,
        )
    })
}

/// Decode `bytes` (an MP3 stream) into interleaved `f32` PCM samples.
///
/// Symphonia returns each decoded packet as a planar
/// [`GenericAudioBufferRef`]; this helper interleaves the channels so
/// the result is a single `Vec<f32>` suitable for both the shared
/// [`super::redact::apply`] helper and for handing back to
/// [`encode_from_pcm`].
pub(super) fn decode_to_pcm(bytes: &Bytes) -> Result<DecodedMp3, Error> {
    let mss = MediaSourceStream::new(
        Box::new(Cursor::new(bytes.clone())),
        Default::default(),
    );
    let mut hint = Hint::new();
    hint.with_extension("mp3");

    let mut reader = get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| Error::validation(format!("MP3 probe failed: {e}"), TARGET))?;

    let track = reader
        .default_track(TrackType::Audio)
        .ok_or_else(|| Error::validation("MP3 stream has no audio track", TARGET))?;
    let track_id = track.id;

    let audio_params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| Error::validation("MP3 track is missing audio codec params", TARGET))?
        .clone();

    let sample_rate = audio_params.sample_rate.ok_or_else(|| {
        Error::validation("MP3 track is missing a sample rate", TARGET)
    })?;
    let channels = audio_params
        .channels
        .as_ref()
        .map(|c| c.count())
        .ok_or_else(|| Error::validation("MP3 track is missing channel info", TARGET))?;
    let channels_u16 = u16::try_from(channels).map_err(|_| {
        Error::validation(
            format!("MP3 track channel count {channels} exceeds u16"),
            TARGET,
        )
    })?;

    let mut decoder = get_codecs()
        .make_audio_decoder(&audio_params, &AudioDecoderOptions::default())
        .map_err(|e| Error::validation(format!("MP3 decoder init failed: {e}"), TARGET))?;

    let mut samples = Vec::<f32>::new();
    let mut dropped_packets: u64 = 0;
    loop {
        let packet = match reader.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(SymError::IoError(io_err))
                if io_err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => {
                return Err(Error::validation(
                    format!("MP3 packet read failed: {e}"),
                    TARGET,
                ));
            }
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf_ref) => append_interleaved_f32(&buf_ref, channels, &mut samples),
            Err(SymError::DecodeError(_)) => {
                // Single-packet decode failures don't fatally end the
                // stream; symphonia's reference player skips them and
                // continues. Match that behaviour so a single corrupt
                // frame doesn't abort the redact pass — but count the
                // drops so the caller can correlate the shorter output
                // with the input damage.
                dropped_packets += 1;
                continue;
            }
            Err(e) => {
                return Err(Error::validation(
                    format!("MP3 decode failed: {e}"),
                    TARGET,
                ));
            }
        }
    }

    if dropped_packets > 0 {
        // Each dropped packet removes ~1152 samples (MPEG layer 3
        // frame). Surface this so support investigations can
        // correlate a shorter-than-expected output with input damage
        // and so callers can reject the redact result if drift is
        // unacceptable.
        tracing::warn!(
            target: TARGET,
            dropped_packets,
            decoded_samples = samples.len(),
            "decoded MP3 had corrupt packets; redacted output will be shorter than input",
        );
    }

    Ok(DecodedMp3 {
        samples,
        sample_rate,
        channels: channels_u16,
    })
}

fn append_interleaved_f32(
    buf_ref: &GenericAudioBufferRef<'_>,
    channels: usize,
    out: &mut Vec<f32>,
) {
    use symphonia::core::audio::conv::ConvertibleSample;

    fn extend<S: ConvertibleSample + Copy>(
        buf: &symphonia::core::audio::AudioBuffer<S>,
        channels: usize,
        out: &mut Vec<f32>,
    ) where
        f32: symphonia::core::audio::conv::FromSample<S>,
    {
        let frames = buf.frames();
        out.reserve(frames * channels);
        for frame in 0..frames {
            for ch in 0..channels {
                let plane = buf.plane(ch).expect("plane for known channel index");
                let sample = plane[frame];
                out.push(<f32 as symphonia::core::audio::conv::FromSample<S>>::from_sample(sample));
            }
        }
    }

    match buf_ref {
        GenericAudioBufferRef::U8(buf) => extend(buf, channels, out),
        GenericAudioBufferRef::U16(buf) => extend(buf, channels, out),
        GenericAudioBufferRef::U32(buf) => extend(buf, channels, out),
        GenericAudioBufferRef::S8(buf) => extend(buf, channels, out),
        GenericAudioBufferRef::S16(buf) => extend(buf, channels, out),
        GenericAudioBufferRef::S32(buf) => extend(buf, channels, out),
        GenericAudioBufferRef::F32(buf) => extend(buf, channels, out),
        GenericAudioBufferRef::F64(buf) => extend(buf, channels, out),
        // MP3 decoders only output 8-/16-/32-bit integer or 32-/64-bit
        // float samples — never 24-bit. If symphonia ever changes that
        // we'd want to handle the new variants explicitly, not silently
        // route them through a generic path that was never tested.
        GenericAudioBufferRef::U24(_) | GenericAudioBufferRef::S24(_) => {
            unreachable!("MP3 decoder does not emit 24-bit sample buffers");
        }
    }
}

/// Encode `samples` (interleaved `f32` PCM) back to MP3 bytes.
///
/// `target_bitrate_bps` is the desired average bitrate in bits/sec.
/// It's snapped to the nearest [`mp3lame_encoder::Bitrate`] variant
/// LAME accepts; passing the input file's measured average bitrate
/// keeps the round-trip approximately size-stable.
pub(super) fn encode_from_pcm(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
    target_bitrate_bps: u32,
) -> Result<Vec<u8>, Error> {
    let bitrate = snap_bitrate(target_bitrate_bps);

    let mut encoder = Builder::new()
        .ok_or_else(|| Error::validation("LAME builder failed", TARGET))?;
    encoder
        .set_sample_rate(sample_rate)
        .map_err(|e| Error::validation(format!("LAME sample-rate rejected: {e:?}"), TARGET))?;
    encoder
        .set_num_channels(channels as u8)
        .map_err(|e| Error::validation(format!("LAME channel count rejected: {e:?}"), TARGET))?;
    encoder
        .set_brate(bitrate)
        .map_err(|e| Error::validation(format!("LAME bitrate rejected: {e:?}"), TARGET))?;
    encoder
        .set_quality(mp3lame_encoder::Quality::Good)
        .map_err(|e| Error::validation(format!("LAME quality rejected: {e:?}"), TARGET))?;

    let mut encoder = encoder
        .build()
        .map_err(|e| Error::validation(format!("LAME init failed: {e:?}"), TARGET))?;

    let frames = samples.len() / channels as usize;
    let mut out = Vec::<u8>::with_capacity(mp3lame_encoder::max_required_buffer_size(frames));

    match channels {
        1 => {
            encoder
                .encode_to_vec(MonoPcm(samples), &mut out)
                .map_err(|e| Error::validation(format!("LAME encode failed: {e:?}"), TARGET))?;
        }
        2 => {
            encoder
                .encode_to_vec(InterleavedPcm(samples), &mut out)
                .map_err(|e| Error::validation(format!("LAME encode failed: {e:?}"), TARGET))?;
        }
        n => {
            return Err(Error::validation(
                format!("LAME supports 1 or 2 channels, got {n}"),
                TARGET,
            ));
        }
    }
    encoder
        .flush_to_vec::<FlushNoGap>(&mut out)
        .map_err(|e| Error::validation(format!("LAME flush failed: {e:?}"), TARGET))?;

    Ok(out)
}

/// Snap an arbitrary bits-per-second target to the nearest
/// [`mp3lame_encoder::Bitrate`] variant LAME accepts.
fn snap_bitrate(bps: u32) -> mp3lame_encoder::Bitrate {
    use mp3lame_encoder::Bitrate::*;

    // LAME's permissible bitrates. Variant tag value equals the kbps.
    const ALL: &[mp3lame_encoder::Bitrate] = &[
        Kbps8, Kbps16, Kbps24, Kbps32, Kbps40, Kbps48, Kbps64, Kbps80, Kbps96, Kbps112, Kbps128,
        Kbps160, Kbps192, Kbps224, Kbps256, Kbps320,
    ];

    let kbps = (bps + 500) / 1_000; // round to nearest kbps
    ALL.iter()
        .copied()
        .min_by_key(|b| ((*b as u32) as i64 - kbps as i64).abs())
        .unwrap_or(Kbps128)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_bitrate_picks_nearest() {
        use mp3lame_encoder::Bitrate;

        assert_eq!(snap_bitrate(127_000) as u32, Bitrate::Kbps128 as u32);
        assert_eq!(snap_bitrate(96_000) as u32, Bitrate::Kbps96 as u32);
        assert_eq!(snap_bitrate(0) as u32, Bitrate::Kbps8 as u32);
        assert_eq!(snap_bitrate(1_000_000) as u32, Bitrate::Kbps320 as u32);
    }

    #[test]
    fn snap_bitrate_resolves_ties_toward_lower() {
        // 144_000 bps is exactly halfway between Kbps128 and Kbps160.
        // `min_by_key` returns the first minimum seen in iteration
        // order, so the lower variant wins. Locking this in so future
        // refactors don't silently flip the tie-break direction.
        assert_eq!(
            snap_bitrate(144_000) as u32,
            mp3lame_encoder::Bitrate::Kbps128 as u32
        );
    }

    #[test]
    fn snap_bitrate_clamps_below_floor() {
        use mp3lame_encoder::Bitrate;

        // Below the lowest valid bitrate (Kbps8 = 8 kbps) should still
        // produce Kbps8 — degraded inputs (e.g. duration probe glitch)
        // shouldn't crash the encoder.
        assert_eq!(snap_bitrate(7_999) as u32, Bitrate::Kbps8 as u32);
        assert_eq!(snap_bitrate(7_500) as u32, Bitrate::Kbps8 as u32);
    }

    #[test]
    fn round_trips_silence() {
        // Synthesise 0.5s of mono silence at 16 kHz, encode, decode, compare length.
        let samples = vec![0f32; 8_000];
        let encoded = encode_from_pcm(&samples, 16_000, 1, 64_000).unwrap();
        assert!(!encoded.is_empty(), "encode produced no bytes");
        let decoded = decode_to_pcm(&Bytes::from(encoded)).unwrap();
        assert_eq!(decoded.sample_rate, 16_000);
        assert_eq!(decoded.channels, 1);
        // LAME adds encoder delay/padding so the decoded length isn't
        // exactly equal to the input, but should land within roughly
        // ±2 MP3 frames worth of samples (~2300 samples at 16 kHz).
        let diff = (decoded.samples.len() as i64 - samples.len() as i64).abs();
        assert!(
            diff < 2_400,
            "round-trip drift too large: input {}, decoded {}",
            samples.len(),
            decoded.samples.len()
        );
    }
}
