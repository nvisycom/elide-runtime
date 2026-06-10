//! MP3 handler: holds raw MP3 audio bytes and exposes them as a
//! single-track audio handle via [`Handle<Audio>`].
//!
//! Redaction is **not supported**: no pure-Rust MP3 encoder exists
//! and pulling a C dependency (libmp3lame) is out of scope here.
//! [`IndexedHandle::redact`] returns an error. Convert audio to
//! WAV upstream if redaction is required.
//!
//! [`Handle<Audio>`]: crate::core::Handle
//! [`IndexedHandle::redact`]: crate::core::IndexedHandle::redact

use std::sync::Arc;

use bytes::Bytes;
use nvisy_core::Error;
use nvisy_core::modality::{Audio, AudioData, AudioLocation};
use nvisy_core::primitive::TimeSpan;
use nvisy_core::redaction::Redactions;

use super::Mp3Loader;
use crate::content::{ContentData, ContentSource};
use crate::core::{Chunk, Handle, Handler, IndexedHandle, ModalityKind};
use crate::{Format, FormatId, LoaderAdapter};

const TARGET: &str = "mp3-handler";

/// Stable [`FormatId`] for the MP3 codec.
pub const FORMAT_ID: FormatId = FormatId::from_static("nvisy.audio.mp3");

/// [`Format`] descriptor registered into [`crate::CodecRegistry`].
pub fn format() -> Format {
    Format {
        id: FORMAT_ID.clone(),
        modality: ModalityKind::Audio,
        extensions: vec!["mp3".into()],
        content_types: vec!["audio/mpeg".into()],
        loader: Arc::new(LoaderAdapter::new(Mp3Loader)),
    }
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

impl Handler for Mp3Handler {
    fn format(&self) -> FormatId {
        FORMAT_ID.clone()
    }

    fn source(&self) -> &ContentSource {
        &self.source
    }

    #[tracing::instrument(name = "mp3.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<ContentData, Error> {
        tracing::Span::current().record("output_bytes", self.bytes.len());
        let source = ContentSource::new().with_parent(&self.source);
        Ok(ContentData::new(source, self.bytes.clone()))
    }
}

#[async_trait::async_trait]
impl Handle<Audio> for Mp3Handler {
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
}

#[async_trait::async_trait]
impl IndexedHandle<Audio> for Mp3Handler {
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
}
