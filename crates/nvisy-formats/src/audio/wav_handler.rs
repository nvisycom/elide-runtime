//! WAV handler: holds raw WAV audio bytes and provides location-based
//! access via [`AudioHandler`].
//!
//! Redaction decodes the WAV via [`hound`], applies a single
//! sample-level mutation, and re-encodes back to bytes.
//! Supported formats are `i8` / `i16` / `i32` PCM and `f32` IEEE
//! float; other bit depths surface a clear error.
//!
//! Batched redaction goes through [`AudioHandler::redact`], which
//! sorts right-to-left by `time_span.start_us` so
//! [`AudioOutput::Remove`] operations don't shift the indices of
//! pending redactions.
//!
//! [`AudioHandler`]: nvisy_codec::handler::AudioHandler
//! [`AudioHandler::redact`]: nvisy_codec::handler::AudioHandler::redact
//! [`AudioOutput::Remove`]: nvisy_codec::handler::AudioOutput::Remove

use std::io::Cursor;

use bytes::Bytes;
use hound::{Sample, SampleFormat, WavReader, WavSpec, WavWriter};
use nvisy_codec::document::{Located, LocationStream};
use nvisy_codec::handler::{
    AudioData, AudioHandler, AudioRedaction, Handler, apply_audio_redaction,
};
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::{AudioFormat, DocumentType};
use nvisy_ontology::entity::AudioLocation;
use nvisy_ontology::primitive::TimeSpan;

const TARGET: &str = "wav-handler";

/// Handler for loaded WAV content.
#[derive(Debug)]
pub struct WavHandler {
    source: ContentSource,
    bytes: Bytes,
}

impl WavHandler {
    /// Create a handler from raw WAV bytes.
    pub fn new(bytes: Bytes) -> Self {
        Self {
            source: ContentSource::new(),
            bytes,
        }
    }

    /// Set the content source for lineage tracking.
    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }

    /// Reference to the raw audio bytes.
    pub fn bytes(&self) -> &Bytes {
        &self.bytes
    }
}

impl Handler for WavHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Audio(AudioFormat::Wav)
    }

    fn source(&self) -> ContentSource {
        self.source
    }

    #[tracing::instrument(name = "wav.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<ContentData, Error> {
        tracing::Span::current().record("output_bytes", self.bytes.len());
        let source = ContentSource::new().with_parent(&self.source);
        Ok(ContentData::new(source, self.bytes.clone()))
    }
}

#[async_trait::async_trait]
impl AudioHandler for WavHandler {
    fn locations(&self) -> LocationStream<'_, AudioLocation> {
        // Single-track audio: the entire audio as one location with a
        // time span covering the full duration. Duration is unknown
        // without decoding — use 0..0 as a placeholder. The actual
        // time span is set by the STT extraction operation after
        // transcription.
        let location = AudioLocation::new(TimeSpan::new(0, 0));
        LocationStream::new(futures::stream::iter(std::iter::once(Located::new(
            self.source,
            location,
        ))))
    }

    async fn read(&self, _location: &AudioLocation) -> Option<AudioData> {
        // Full audio segment: extracting a sub-segment by time span
        // requires decoding, which we don't do here.
        Some(AudioData::new(self.bytes.clone()))
    }

    async fn redact_at(
        &mut self,
        location: &AudioLocation,
        redaction: AudioRedaction,
    ) -> Result<(), Error> {
        let spec = read_spec(&self.bytes)?;
        let new_bytes = match (spec.sample_format, spec.bits_per_sample) {
            (SampleFormat::Int, 8) => {
                redact_typed::<i8>(&self.bytes, spec, location.time_span, &redaction)?
            }
            (SampleFormat::Int, 16) => {
                redact_typed::<i16>(&self.bytes, spec, location.time_span, &redaction)?
            }
            (SampleFormat::Int, 24 | 32) => {
                redact_typed::<i32>(&self.bytes, spec, location.time_span, &redaction)?
            }
            (SampleFormat::Float, 32) => {
                redact_typed::<f32>(&self.bytes, spec, location.time_span, &redaction)?
            }
            _ => {
                return Err(Error::validation(
                    format!(
                        "WAV format not yet supported: {:?}/{} bits",
                        spec.sample_format, spec.bits_per_sample
                    ),
                    TARGET,
                ));
            }
        };
        self.bytes = Bytes::from(new_bytes);
        Ok(())
    }
}

/// Read just the WAV header to discover the sample format.
fn read_spec(bytes: &Bytes) -> Result<WavSpec, Error> {
    let reader = WavReader::new(Cursor::new(bytes.as_ref()))
        .map_err(|e| Error::validation(format!("invalid WAV: {e}"), TARGET))?;
    Ok(reader.spec())
}

/// Decode → redact → re-encode for a specific sample type.
fn redact_typed<S>(
    bytes: &Bytes,
    spec: WavSpec,
    time_span: TimeSpan,
    redaction: &AudioRedaction,
) -> Result<Vec<u8>, Error>
where
    S: Sample + Default + Clone,
{
    let mut reader = WavReader::new(Cursor::new(bytes.as_ref()))
        .map_err(|e| Error::validation(format!("invalid WAV: {e}"), TARGET))?;
    let mut samples: Vec<S> = reader
        .samples::<S>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| Error::validation(format!("WAV sample decode error: {e}"), TARGET))?;

    apply_audio_redaction(
        &mut samples,
        time_span,
        redaction,
        spec.sample_rate,
        spec.channels,
    );

    let mut buf = Cursor::new(Vec::<u8>::new());
    {
        let mut writer = WavWriter::new(&mut buf, spec)
            .map_err(|e| Error::validation(format!("WAV writer init error: {e}"), TARGET))?;
        for sample in samples {
            writer
                .write_sample(sample)
                .map_err(|e| Error::validation(format!("WAV sample write error: {e}"), TARGET))?;
        }
        writer
            .finalize()
            .map_err(|e| Error::validation(format!("WAV finalize error: {e}"), TARGET))?;
    }
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use hound::SampleFormat;
    use nvisy_codec::handler::{AudioHandler, AudioOutput, ConflictPolicy, Redactions};

    use super::*;

    /// Encode a mono i16 PCM WAV with the given samples at 1 kHz.
    fn encode_wav_mono_i16(samples: &[i16]) -> Bytes {
        let spec = WavSpec {
            channels: 1,
            sample_rate: 1000,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut buf = Cursor::new(Vec::<u8>::new());
        {
            let mut writer = WavWriter::new(&mut buf, spec).unwrap();
            for &s in samples {
                writer.write_sample(s).unwrap();
            }
            writer.finalize().unwrap();
        }
        Bytes::from(buf.into_inner())
    }

    fn decode_wav_mono_i16(bytes: &Bytes) -> Vec<i16> {
        let mut reader = WavReader::new(Cursor::new(bytes.as_ref())).unwrap();
        reader.samples::<i16>().map(Result::unwrap).collect()
    }

    #[tokio::test]
    async fn silence_zeros_samples_in_range() {
        // 10 samples at 1 kHz = 10 ms.
        let bytes = encode_wav_mono_i16(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let mut handler = WavHandler::new(bytes);

        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            AudioLocation::new(TimeSpan::new(3_000, 6_000)),
            AudioRedaction::new(AudioOutput::Silence),
        )
        .unwrap();
        handler.redact(rs).await.unwrap();

        let samples = decode_wav_mono_i16(handler.bytes());
        assert_eq!(samples, vec![1, 2, 3, 0, 0, 0, 7, 8, 9, 10]);
    }

    #[tokio::test]
    async fn remove_shortens_file() {
        let bytes = encode_wav_mono_i16(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let mut handler = WavHandler::new(bytes);

        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            AudioLocation::new(TimeSpan::new(3_000, 6_000)),
            AudioRedaction::new(AudioOutput::Remove),
        )
        .unwrap();
        handler.redact(rs).await.unwrap();

        let samples = decode_wav_mono_i16(handler.bytes());
        assert_eq!(samples, vec![1, 2, 3, 7, 8, 9, 10]);
    }

    #[tokio::test]
    async fn multiple_removes_apply_right_to_left() {
        // Two non-overlapping Remove redactions:
        //   [1..3) removes samples 1..3 (values 2, 3)
        //   [6..8) removes samples 6..8 (values 7, 8)
        // Both time spans measured against original 10-sample buffer.
        let bytes = encode_wav_mono_i16(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let mut handler = WavHandler::new(bytes);

        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            AudioLocation::new(TimeSpan::new(1_000, 3_000)),
            AudioRedaction::new(AudioOutput::Remove),
        )
        .unwrap();
        rs.try_insert(
            AudioLocation::new(TimeSpan::new(6_000, 8_000)),
            AudioRedaction::new(AudioOutput::Remove),
        )
        .unwrap();
        handler.redact(rs).await.unwrap();

        let samples = decode_wav_mono_i16(handler.bytes());
        // After right-to-left: remove 7,8 first → [1,2,3,4,5,6,9,10],
        // then remove 2,3 → [1,4,5,6,9,10].
        assert_eq!(samples, vec![1, 4, 5, 6, 9, 10]);
    }

    #[tokio::test]
    async fn empty_redactions_is_noop() {
        let bytes = encode_wav_mono_i16(&[1, 2, 3]);
        let original = bytes.clone();
        let mut handler = WavHandler::new(bytes);

        let rs: Redactions<AudioLocation, AudioRedaction> = Redactions::default();
        handler.redact(rs).await.unwrap();
        assert_eq!(handler.bytes(), &original);
    }

    #[tokio::test]
    async fn unsupported_format_returns_error() {
        // Bogus bytes — not a real WAV. read_spec fails.
        let mut handler = WavHandler::new(Bytes::from_static(b"not-a-wav"));
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            AudioLocation::new(TimeSpan::new(0, 1_000)),
            AudioRedaction::new(AudioOutput::Silence),
        )
        .unwrap();
        let err = handler.redact(rs).await.unwrap_err();
        assert!(err.to_string().contains("invalid WAV"));
    }
}
