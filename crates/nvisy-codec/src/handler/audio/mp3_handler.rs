//! MP3 handler: holds raw MP3 audio bytes and provides span-based
//! access via [`Handler`] + [`AudioHandler`].
//!
//! # Span model
//!
//! [`AudioHandler::audio_spans`] yields a single [`Span`] carrying the
//! entire audio payload as [`AudioData`].  [`AudioHandler::edit_audio`]
//! replaces the payload from the first incoming edit.

use bytes::Bytes;
use futures::StreamExt;
use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;

use super::AudioData;
use crate::handler::{AudioHandler, Handler, Span, SpanEditStream, SpanStream};
use crate::transform::{AudioRedact, AudioRedaction};

#[derive(Debug)]
pub struct Mp3Handler {
    pub(crate) source: ContentSource,
    pub(crate) bytes: Bytes,
}

impl Mp3Handler {
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

    pub fn bytes(&self) -> &Bytes {
        &self.bytes
    }
}

impl Handler for Mp3Handler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Mp3
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
    type AudioId = ();

    async fn audio_spans(&self) -> SpanStream<'_, (), AudioData> {
        SpanStream::new(futures::stream::iter(std::iter::once(Span::new(
            (),
            AudioData::new(self.bytes.clone()),
        ))))
    }

    async fn edit_audio(&mut self, edits: SpanEditStream<'_, (), AudioData>) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        if let Some(edit) = edits.into_iter().next() {
            self.bytes = edit.data.into_inner();
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl AudioRedact for Mp3Handler {
    async fn redact_audio(&mut self, _redactions: &[AudioRedaction]) -> Result<(), Error> {
        tracing::warn!("MP3 audio redaction is not yet implemented");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::{AudioHandler, SpanEdit};

    #[tokio::test]
    async fn view_spans_returns_single_span() {
        let h = Mp3Handler::new(Bytes::from_static(b"ID3-mp3-data"));
        let spans: Vec<_> = h.audio_spans().await.collect().await;
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].data.as_bytes().as_ref(), b"ID3-mp3-data");
    }

    #[tokio::test]
    async fn edit_spans_replaces_bytes() -> Result<(), Error> {
        let mut h = Mp3Handler::new(Bytes::from_static(b"original"));
        h.edit_audio(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit::new((), AudioData::new(Bytes::from_static(b"replaced"))),
        ])))
        .await?;
        assert_eq!(h.bytes().as_ref(), b"replaced");
        Ok(())
    }

    #[test]
    fn encode_returns_current_bytes() -> Result<(), Error> {
        let h = Mp3Handler::new(Bytes::from_static(b"audio-data"));
        let encoded = h.encode()?;
        assert_eq!(encoded.as_bytes(), b"audio-data");
        Ok(())
    }
}
