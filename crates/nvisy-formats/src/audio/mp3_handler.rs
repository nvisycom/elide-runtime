//! MP3 handler: holds raw MP3 audio bytes and provides location-based
//! access via [`Handle`].
//!
//! Redaction is **not supported**: no pure-Rust MP3 encoder exists and
//! pulling a C dependency (libmp3lame) is out of scope here. Callers
//! get an explicit error from [`Handle::redact_at`]; under
//! [`Handle::redact`] this aborts the document's pipeline at the
//! first redaction. Convert to WAV upstream if audio redaction is
//! required.
//!
//! [`Handle`]: nvisy_codec::core::Handle
//! [`Handle::redact_at`]: nvisy_codec::core::Handle::redact_at
//! [`Handle::redact`]: nvisy_codec::core::Handle::redact

use bytes::Bytes;
use nvisy_codec::core::{Handle, Located, LocationStream, Redactions};
use nvisy_codec::handler::{AudioData, AudioRedaction, Handler, sort_redactions_for_audio};
use nvisy_core::Error;
use nvisy_core::content::{AudioFormat, ContentData, ContentSource, DocumentType};
use nvisy_ontology::modality::Audio;
use nvisy_ontology::primitive::TimeSpan;

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
impl Handle<Audio> for Mp3Handler {
    fn locations(&self) -> LocationStream<'_, Audio> {
        let location = Audio::new(TimeSpan::new(0, 0));
        LocationStream::new(futures::stream::iter(std::iter::once(Located::new(
            self.source,
            location,
        ))))
    }

    async fn read(&self, _location: &Audio) -> Option<AudioData> {
        Some(AudioData::new(self.bytes.clone()))
    }

    async fn redact_at(
        &mut self,
        _location: &Audio,
        _redaction: AudioRedaction,
    ) -> Result<(), Error> {
        Err(Error::validation(
            "MP3 redaction is not yet supported — convert audio to WAV before redaction",
            TARGET,
        ))
    }

    /// Override the default loop to apply spans right-to-left so a
    /// removal doesn't invalidate earlier sample indices.
    async fn redact(&mut self, redactions: Redactions<Audio, AudioRedaction>) -> Result<(), Error> {
        for (location, redaction) in sort_redactions_for_audio(redactions) {
            self.redact_at(&location, redaction).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use nvisy_codec::core::{Handle, Redactions};
    use nvisy_codec::handler::AudioOutput;
    use nvisy_ontology::primitive::TimeSpan;

    use super::*;

    #[tokio::test]
    async fn redact_with_entries_errors() {
        let mut handler = Mp3Handler::new(Bytes::from_static(b"fake mp3"));
        let location = Audio::new(TimeSpan::new(0, 1_000));
        let mut rs = Redactions::new();
        rs.push(location, AudioRedaction::new(AudioOutput::Silence));
        let err = handler.redact(rs).await.unwrap_err();
        assert!(
            err.to_string()
                .contains("MP3 redaction is not yet supported")
        );
    }

    #[tokio::test]
    async fn empty_redactions_is_noop() {
        let mut handler = Mp3Handler::new(Bytes::from_static(b"fake mp3"));
        let rs: Redactions<Audio, AudioRedaction> = Redactions::default();
        handler.redact(rs).await.unwrap();
    }
}
