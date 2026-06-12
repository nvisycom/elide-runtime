//! MP3 handler: holds raw MP3 audio bytes and exposes them as a
//! single-track audio handle via [`Handler<Audio>`].
//!
//! Redaction is **not supported**: no pure-Rust MP3 encoder exists
//! and pulling a C dependency (libmp3lame) is out of scope here.
//! [`Handler::redact`] returns an error. Convert audio to
//! WAV upstream if redaction is required.
//!
//! [`Handler<Audio>`]: crate::Handler
//! [`Handler::redact`]: crate::Handler::redact

use bytes::Bytes;
use nvisy_core::Error;
use nvisy_core::modality::{Audio, AudioData, AudioLocation};
use nvisy_core::primitive::TimeSpan;
use nvisy_core::redaction::Redactions;

use super::Mp3Loader;
use super::duration::probe_duration_us;
use crate::content::{ContentData, ContentSource};
use crate::{Chunk, Format, FormatId, Handler};

const TARGET: &str = "nvisy_codec::handler::audio::mp3";

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
        Ok(Some(Chunk { location, data }))
    }

    async fn read(&self, _location: &AudioLocation) -> Result<Option<AudioData>, Error> {
        Ok(Some(
            AudioData::new(self.bytes.clone()).with_filename(self.filename.clone()),
        ))
    }

    async fn redact(&mut self, redactions: Redactions<Audio>) -> Result<(), Error> {
        if redactions.is_empty() {
            return Ok(());
        }
        Err(Error::validation(
            "MP3 redaction is not yet supported — convert audio to WAV before redaction",
            TARGET,
        ))
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::redaction::AudioReplacement;

    use super::*;

    #[tokio::test]
    async fn redact_with_entries_errors() {
        let mut handler = Mp3Handler::new(Bytes::from_static(b"fake mp3"));
        let location = AudioLocation::new(TimeSpan::new(0, 1_000));
        let mut rs = Redactions::new();
        rs.push(location, AudioReplacement::Silence);
        let err = handler.redact(rs).await.unwrap_err();
        assert!(
            err.to_string()
                .contains("MP3 redaction is not yet supported")
        );
    }

    #[tokio::test]
    async fn empty_redactions_is_noop() {
        let mut handler = Mp3Handler::new(Bytes::from_static(b"fake mp3"));
        let rs: Redactions<Audio> = Redactions::default();
        handler.redact(rs).await.unwrap();
    }

    #[tokio::test]
    async fn next_chunk_propagates_probe_error_for_garbage_bytes() {
        // Real MP3 fixtures live in upstream symphonia tests; here we
        // only need to confirm the handler wires the probe in and
        // surfaces failures rather than silently stamping (0, 0).
        let mut handler = Mp3Handler::new(Bytes::from_static(b"definitely not an mp3"));
        let err = handler.next_chunk().await.unwrap_err();
        assert!(err.to_string().contains("audio probe failed"));
    }
}
