//! MP3 handler: holds raw MP3 audio bytes and provides location-based
//! access via [`AudioHandler`].
//!
//! Redaction is **not supported**: no pure-Rust MP3 encoder exists and
//! pulling a C dependency (libmp3lame) is out of scope here. Callers
//! get an explicit error from [`AudioHandler::redact_at`]; under
//! [`AudioTransform::redact`] this aborts the document's pipeline at
//! the first redaction. Convert to WAV upstream if audio redaction is
//! required.
//!
//! [`AudioHandler`]: crate::handler::AudioHandler
//! [`AudioHandler::redact_at`]: crate::handler::AudioHandler::redact_at
//! [`AudioTransform::redact`]: crate::transform::AudioTransform::redact

use bytes::Bytes;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::{AudioFormat, DocumentType};
use nvisy_ontology::entity::AudioLocation;
use nvisy_ontology::primitive::TimeSpan;

use crate::document::{Located, LocationStream};
use crate::handler::{AudioData, AudioHandler, Handler};
use crate::transform::AudioRedaction;

const TARGET: &str = "mp3-handler";

/// Handler for loaded MP3 content.
#[derive(Debug)]
pub struct Mp3Handler {
    source: ContentSource,
    bytes: Bytes,
}

impl Mp3Handler {
    /// Create a handler from raw MP3 bytes.
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

impl Handler for Mp3Handler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Audio(AudioFormat::Mp3)
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
}

#[async_trait::async_trait]
impl AudioHandler for Mp3Handler {
    fn locations(&self) -> LocationStream<'_, AudioLocation> {
        let location = AudioLocation {
            time_span: TimeSpan {
                start_us: 0,
                end_us: 0,
            },
            speaker_id: None,
            audio_id: None,
        };
        LocationStream::new(futures::stream::iter(std::iter::once(Located::new(
            self.source,
            location,
        ))))
    }

    async fn read(&self, _location: &AudioLocation) -> Option<AudioData> {
        Some(AudioData::new(self.bytes.clone()))
    }

    async fn redact_at(
        &mut self,
        _location: &AudioLocation,
        _redaction: AudioRedaction,
    ) -> Result<(), Error> {
        Err(Error::validation(
            "MP3 redaction is not yet supported — convert audio to WAV before redaction",
            TARGET,
        ))
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::primitive::TimeSpan;

    use super::*;
    use crate::transform::{AudioOutput, AudioTransform, ConflictPolicy, Redactions};

    #[tokio::test]
    async fn redact_with_entries_errors() {
        let mut handler = Mp3Handler::new(Bytes::from_static(b"fake mp3"));
        let location = AudioLocation {
            time_span: TimeSpan { start_us: 0, end_us: 1_000 },
            speaker_id: None,
            audio_id: None,
        };
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(location, AudioRedaction::new(AudioOutput::Silence))
            .unwrap();
        let err = handler.redact(rs).await.unwrap_err();
        assert!(err.to_string().contains("MP3 redaction is not yet supported"));
    }

    #[tokio::test]
    async fn empty_redactions_is_noop() {
        let mut handler = Mp3Handler::new(Bytes::from_static(b"fake mp3"));
        let rs: Redactions<AudioLocation, AudioRedaction> = Redactions::default();
        handler.redact(rs).await.unwrap();
    }
}
