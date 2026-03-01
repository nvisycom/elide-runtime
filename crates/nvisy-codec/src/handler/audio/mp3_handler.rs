//! MP3 handler: holds raw MP3 audio bytes and provides span-based
//! access via [`Handler`].
//!
//! # Span model
//!
//! [`Handler::view_spans`] yields a single [`Span`] carrying the
//! entire audio payload as [`Bytes`].  [`Handler::edit_spans`]
//! replaces the payload from the first incoming edit.

use bytes::Bytes;
use futures::StreamExt;

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;

use crate::stream::{SpanEditStream, SpanStream};
use crate::handler::{Handler, Span};
use crate::transform::{AudioHandler, AudioRedaction};

#[derive(Debug, Clone)]
pub struct Mp3Handler {
    pub(crate) bytes: Bytes,
}

impl Mp3Handler {
    pub fn new(bytes: Bytes) -> Self {
        Self { bytes }
    }

    pub fn bytes(&self) -> &Bytes {
        &self.bytes
    }
}

#[async_trait::async_trait]
impl Handler for Mp3Handler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Mp3
    }

    #[tracing::instrument(name = "mp3.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<Bytes, Error> {
        tracing::Span::current().record("output_bytes", self.bytes.len());
        Ok(self.bytes.clone())
    }

    type SpanId = ();
    type SpanData = Bytes;

    async fn view_spans(&self) -> SpanStream<'_, (), Bytes> {
        SpanStream::new(futures::stream::iter(std::iter::once(
            Span::new((), self.bytes.clone()),
        )))
    }

    async fn edit_spans(
        &mut self,
        edits: SpanEditStream<'_, (), Bytes>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        if let Some(edit) = edits.into_iter().next() {
            self.bytes = edit.data;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl AudioHandler for Mp3Handler {
    async fn redact_spans(&mut self, _redactions: &[AudioRedaction]) -> Result<(), Error> {
        tracing::warn!("MP3 audio redaction is not yet implemented");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::SpanEdit;

    #[tokio::test]
    async fn view_spans_returns_single_span() {
        let h = Mp3Handler::new(Bytes::from_static(b"ID3-mp3-data"));
        let spans: Vec<_> = h.view_spans().await.collect().await;
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].data.as_ref(), b"ID3-mp3-data");
    }

    #[tokio::test]
    async fn edit_spans_replaces_bytes() -> Result<(), Error> {
        let mut h = Mp3Handler::new(Bytes::from_static(b"original"));
        let replacement = Bytes::from_static(b"replaced");
        h.edit_spans(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit::new((), replacement.clone()),
        ])))
        .await?;
        assert_eq!(h.bytes().as_ref(), b"replaced");
        Ok(())
    }

    #[test]
    fn encode_returns_current_bytes() -> Result<(), Error> {
        let h = Mp3Handler::new(Bytes::from_static(b"audio-data"));
        let encoded = h.encode()?;
        assert_eq!(&encoded[..], b"audio-data");
        Ok(())
    }
}
