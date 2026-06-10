//! WAV handler: holds raw WAV audio bytes and exposes them as a
//! single-track audio handle via [`Handle<Audio>`].
//!
//! Redaction decodes the WAV via [`hound`], applies sample-level
//! mutations, and re-encodes back to bytes. Supported formats: `i8` /
//! `i16` / `i32` PCM and `f32` IEEE float; other bit depths surface a
//! clear error.
//!
//! Batched [`Handle::redact`] sorts right-to-left by
//! `time_span.start_us` so [`AudioReplacement::Remove`] operations don't
//! shift the indices of pending redactions.
//!
//! [`Handle<Audio>`]: crate::core::Handle
//! [`Handle::redact`]: crate::core::Handle::redact
//! [`AudioReplacement::Remove`]: nvisy_core::redaction::AudioReplacement::Remove

use std::io::Cursor;
use std::sync::Arc;

use bytes::Bytes;
use hound::{Sample, SampleFormat, WavReader, WavSpec, WavWriter};
use nvisy_core::Error;
use nvisy_core::modality::{Audio, AudioData, AudioLocation};
use nvisy_core::primitive::TimeSpan;
use nvisy_core::redaction::{AudioReplacement, Redactions};

use super::{WavLoader, redact};
use crate::content::{ContentData, ContentSource};
use crate::core::{Chunk, Handle, Handler, ModalityKind};
use crate::handler::audio::sort_redactions_for_audio;
use crate::{Format, FormatId, LoaderAdapter};

const TARGET: &str = "wav-handler";

/// Stable [`FormatId`] for the WAV codec.
pub const FORMAT_ID: FormatId = FormatId::from_static("nvisy.audio.wav");

/// [`Format`] descriptor registered into [`crate::CodecRegistry`].
pub fn format() -> Format {
    Format {
        id: FORMAT_ID.clone(),
        modality: ModalityKind::Audio,
        extensions: vec!["wav".into()],
        content_types: vec!["audio/wav".into(), "audio/x-wav".into()],
        loader: Arc::new(LoaderAdapter::new(WavLoader)),
    }
}

/// Handler for loaded WAV content. Stores the encoded bytes; decode
/// happens on demand inside [`Handle::redact`].
///
/// [`Handle::redact`]: crate::core::Handle::redact
#[derive(Debug)]
pub struct WavHandler {
    source: ContentSource,
    bytes: Bytes,
    filename: String,
    yielded: bool,
}

impl WavHandler {
    /// Create a handler from raw WAV bytes.
    pub fn new(bytes: Bytes) -> Self {
        Self {
            source: ContentSource::new(),
            bytes,
            filename: "audio.wav".to_owned(),
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

impl Handler for WavHandler {
    fn format(&self) -> FormatId {
        FORMAT_ID.clone()
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
impl Handle<Audio> for WavHandler {
    async fn next_chunk(&mut self) -> Result<Option<Chunk<Audio>>, Error> {
        if self.yielded {
            return Ok(None);
        }
        let location = AudioLocation::new(TimeSpan::new(0, 0));
        let data = AudioData::new(self.bytes.clone()).with_filename(self.filename.clone());
        self.yielded = true;
        Ok(Some(Chunk {
            location,
            data,
            embed: None,
        }))
    }

    async fn read(&self, _location: &AudioLocation) -> Result<Option<AudioData>, Error> {
        Ok(Some(
            AudioData::new(self.bytes.clone()).with_filename(self.filename.clone()),
        ))
    }

    /// Apply spans right-to-left so a [`AudioReplacement::Remove`] doesn't
    /// invalidate earlier sample indices.
    async fn redact(&mut self, redactions: Redactions<Audio>) -> Result<(), Error> {
        for (location, replacement) in sort_redactions_for_audio(redactions) {
            self.redact_one(&location, replacement)?;
        }
        Ok(())
    }
}

impl WavHandler {
    fn redact_one(
        &mut self,
        location: &AudioLocation,
        replacement: AudioReplacement,
    ) -> Result<(), Error> {
        let spec = read_spec(&self.bytes)?;
        let new_bytes = match (spec.sample_format, spec.bits_per_sample) {
            (SampleFormat::Int, 8) => {
                redact_typed::<i8>(&self.bytes, spec, location.time_span, &replacement)?
            }
            (SampleFormat::Int, 16) => {
                redact_typed::<i16>(&self.bytes, spec, location.time_span, &replacement)?
            }
            (SampleFormat::Int, 24 | 32) => {
                redact_typed::<i32>(&self.bytes, spec, location.time_span, &replacement)?
            }
            (SampleFormat::Float, 32) => {
                redact_typed::<f32>(&self.bytes, spec, location.time_span, &replacement)?
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

fn read_spec(bytes: &Bytes) -> Result<WavSpec, Error> {
    let reader = WavReader::new(Cursor::new(bytes.as_ref()))
        .map_err(|e| Error::validation(format!("invalid WAV: {e}"), TARGET))?;
    Ok(reader.spec())
}

fn redact_typed<S>(
    bytes: &Bytes,
    spec: WavSpec,
    time_span: TimeSpan,
    replacement: &AudioReplacement,
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

    redact::apply(
        &mut samples,
        time_span,
        replacement,
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
    use nvisy_core::redaction::AudioReplacement;

    use super::*;

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
        let bytes = encode_wav_mono_i16(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let mut handler = WavHandler::new(bytes);
        let mut rs = Redactions::new();
        rs.push(
            AudioLocation::new(TimeSpan::new(3_000, 6_000)),
            AudioReplacement::Silence,
        );
        handler.redact(rs).await.unwrap();
        let samples = decode_wav_mono_i16(handler.bytes());
        assert_eq!(samples, vec![1, 2, 3, 0, 0, 0, 7, 8, 9, 10]);
    }

    #[tokio::test]
    async fn remove_shortens_file() {
        let bytes = encode_wav_mono_i16(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let mut handler = WavHandler::new(bytes);
        let mut rs = Redactions::new();
        rs.push(
            AudioLocation::new(TimeSpan::new(3_000, 6_000)),
            AudioReplacement::Remove,
        );
        handler.redact(rs).await.unwrap();
        let samples = decode_wav_mono_i16(handler.bytes());
        assert_eq!(samples, vec![1, 2, 3, 7, 8, 9, 10]);
    }

    #[tokio::test]
    async fn multiple_removes_apply_right_to_left() {
        let bytes = encode_wav_mono_i16(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let mut handler = WavHandler::new(bytes);
        let mut rs = Redactions::new();
        rs.push(
            AudioLocation::new(TimeSpan::new(1_000, 3_000)),
            AudioReplacement::Remove,
        );
        rs.push(
            AudioLocation::new(TimeSpan::new(6_000, 8_000)),
            AudioReplacement::Remove,
        );
        handler.redact(rs).await.unwrap();
        let samples = decode_wav_mono_i16(handler.bytes());
        assert_eq!(samples, vec![1, 4, 5, 6, 9, 10]);
    }

    #[tokio::test]
    async fn empty_redactions_is_noop() {
        let bytes = encode_wav_mono_i16(&[1, 2, 3]);
        let original = bytes.clone();
        let mut handler = WavHandler::new(bytes);
        let rs: Redactions<Audio> = Redactions::default();
        handler.redact(rs).await.unwrap();
        assert_eq!(handler.bytes(), &original);
    }

    #[tokio::test]
    async fn unsupported_format_returns_error() {
        let mut handler = WavHandler::new(Bytes::from_static(b"not-a-wav"));
        let mut rs = Redactions::new();
        rs.push(
            AudioLocation::new(TimeSpan::new(0, 1_000)),
            AudioReplacement::Silence,
        );
        let err = handler.redact(rs).await.unwrap_err();
        assert!(err.to_string().contains("invalid WAV"));
    }
}
