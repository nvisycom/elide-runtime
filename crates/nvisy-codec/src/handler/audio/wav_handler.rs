//! WAV handler: holds raw WAV audio bytes and provides span-based
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
use nvisy_core::fs::{AudioFormat, DocumentType};
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;

use super::{AudioData, AudioSpanId};
use crate::document::{Span, SpanStream};
use crate::handler::{AudioHandler, Handler};

#[derive(Debug)]
pub struct WavHandler {
    pub(crate) source: ContentSource,
    pub(crate) bytes: Bytes,
}

impl WavHandler {
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
    type AudioId = AudioSpanId;

    async fn audio_spans(&self) -> SpanStream<'_, AudioSpanId, AudioData> {
        SpanStream::new(futures::stream::iter(std::iter::once(Span::new(
            AudioSpanId,
            AudioData::new(self.bytes.clone()),
        ))))
    }

    async fn edit_audio(&mut self, edits: SpanStream<'_, AudioSpanId, AudioData>) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        if let Some(edit) = edits.into_iter().next() {
            self.bytes = edit.data.into_inner();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Span;
    use crate::handler::AudioHandler;

    #[tokio::test]
    async fn view_spans_returns_single_span() {
        let h = WavHandler::new(Bytes::from_static(b"RIFF-wav-data"));
        let spans: Vec<_> = h.audio_spans().await.collect().await;
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].data.as_bytes().as_ref(), b"RIFF-wav-data");
    }

    #[tokio::test]
    async fn edit_spans_replaces_bytes() -> Result<(), Error> {
        let mut h = WavHandler::new(Bytes::from_static(b"original"));
        h.edit_audio(SpanStream::new(futures::stream::iter(vec![
            Span::new(AudioSpanId, AudioData::new(Bytes::from_static(b"replaced"))),
        ])))
        .await?;
        assert_eq!(h.bytes().as_ref(), b"replaced");
        Ok(())
    }

    #[test]
    fn encode_returns_current_bytes() -> Result<(), Error> {
        let h = WavHandler::new(Bytes::from_static(b"audio-data"));
        let encoded = h.encode()?;
        assert_eq!(encoded.as_bytes(), b"audio-data");
        Ok(())
    }
}
