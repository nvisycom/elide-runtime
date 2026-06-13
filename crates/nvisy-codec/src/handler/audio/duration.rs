//! Audio clip duration probing via Symphonia.
//!
//! Both [`WavHandler`] and [`Mp3Handler`] hand the whole clip back
//! as a single [`Chunk<Audio>`] from `next_chunk`. The chunk's
//! [`AudioLocation`] needs an honest [`TimeSpan`] so downstream
//! consumers (most importantly the STT extractor) can anchor their
//! per-segment timestamps to absolute clip coordinates.
//!
//! [`probe_duration_us`] runs Symphonia's format probe over the
//! supplied bytes and reads the first audio track's container-level
//! duration. No samples are decoded — the probe stops as soon as
//! the demuxer has seen enough header to populate the track
//! metadata.
//!
//! [`WavHandler`]: super::WavHandler
//! [`Mp3Handler`]: super::Mp3Handler
//! [`Chunk<Audio>`]: crate::Chunk
//! [`AudioLocation`]: nvisy_core::modality::AudioLocation
//! [`TimeSpan`]: nvisy_core::primitive::TimeSpan
//! [`probe_duration_us`]: self::probe_duration_us

use std::io::Cursor;

use bytes::Bytes;
use nvisy_core::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::formats::probe::Hint;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::Timestamp;
use symphonia::default::get_probe;

const TARGET: &str = "nvisy_codec::handler::audio::duration";

/// Probe `bytes` with Symphonia and return the clip duration in
/// microseconds.
///
/// `extension_hint` is a Symphonia format hint (e.g. `"wav"`,
/// `"mp3"`). The hint biases the probe toward the format the caller
/// expects; an incorrect hint does not cause a failure as long as
/// the bytes still match some registered format.
///
/// Errors when the probe cannot find a container format, when the
/// first track lacks a timebase or a known duration, or when the
/// computed duration would overflow `i64` microseconds.
pub(super) fn probe_duration_us(bytes: &Bytes, extension_hint: &str) -> Result<i64, Error> {
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes.clone())), Default::default());

    let mut hint = Hint::new();
    hint.with_extension(extension_hint);

    let reader = get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| Error::validation(format!("audio probe failed: {e}"), TARGET))?;

    let track = reader
        .tracks()
        .first()
        .ok_or_else(|| Error::validation("audio probe returned no tracks", TARGET))?;

    let time_base = track
        .time_base
        .ok_or_else(|| Error::validation("audio track is missing a timebase", TARGET))?;
    let duration = track.duration.ok_or_else(|| {
        Error::validation("audio track is missing a container-level duration", TARGET)
    })?;

    // `Track::duration` is `Duration` (timebase ticks, u64). `TimeBase::calc_time`
    // operates on `Timestamp` (timebase ticks, i64) — same unit, different signedness.
    // Reinterpret as a timestamp anchored at zero.
    let ticks = i64::try_from(duration.get()).map_err(|_| {
        Error::validation(
            format!(
                "audio duration in timebase ticks overflowed i64: {}",
                duration.get()
            ),
            TARGET,
        )
    })?;
    let time = time_base.calc_time(Timestamp::new(ticks)).ok_or_else(|| {
        Error::validation("audio duration overflowed when converted to time", TARGET)
    })?;

    let us = time.as_nanos() / 1_000;
    if us < 0 {
        return Err(Error::validation(
            format!("audio duration is negative: {us} us"),
            TARGET,
        ));
    }
    i64::try_from(us).map_err(|_| {
        Error::validation(
            format!("audio duration overflowed i64 microseconds: {us}"),
            TARGET,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_silence_wav(duration_us: i64, sample_rate: u32) -> Bytes {
        use hound::{SampleFormat, WavSpec, WavWriter};

        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let total_samples = ((duration_us as i128) * (sample_rate as i128) / 1_000_000) as usize;

        let mut buf = Cursor::new(Vec::<u8>::new());
        {
            let mut writer = WavWriter::new(&mut buf, spec).unwrap();
            for _ in 0..total_samples {
                writer.write_sample(0i16).unwrap();
            }
            writer.finalize().unwrap();
        }
        Bytes::from(buf.into_inner())
    }

    #[test]
    fn probes_one_second_wav() {
        let wav = write_silence_wav(1_000_000, 16_000);
        let us = probe_duration_us(&wav, "wav").unwrap();
        // 16_000 samples at 16_000 Hz = exactly 1_000_000 us.
        assert_eq!(us, 1_000_000);
    }

    #[test]
    fn probes_fractional_wav() {
        let wav = write_silence_wav(2_500_000, 8_000);
        let us = probe_duration_us(&wav, "wav").unwrap();
        // 20_000 samples at 8_000 Hz = 2_500_000 us.
        assert_eq!(us, 2_500_000);
    }

    #[test]
    fn rejects_garbage_bytes() {
        let garbage = Bytes::from_static(b"not an audio file");
        let err = probe_duration_us(&garbage, "wav").unwrap_err();
        assert!(err.to_string().contains("audio probe failed"));
    }
}
