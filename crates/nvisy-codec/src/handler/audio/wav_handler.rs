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
use nvisy_core::fs::DocumentType;

use crate::handler::{Handler, Span, SpanEditStream, SpanStream, AudioHandler};
use crate::transform::{AudioRedact, AudioRedaction};
use super::AudioData;

#[derive(Debug, Clone)]
pub struct WavHandler {
    pub(crate) bytes: Bytes,
}

impl WavHandler {
    pub fn new(bytes: Bytes) -> Self {
        Self { bytes }
    }

    pub fn bytes(&self) -> &Bytes {
        &self.bytes
    }
}

impl Handler for WavHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Wav
    }

    #[tracing::instrument(name = "wav.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<Bytes, Error> {
        tracing::Span::current().record("output_bytes", self.bytes.len());
        Ok(self.bytes.clone())
    }
}

#[async_trait::async_trait]
impl AudioHandler for WavHandler {
    type AudioId = ();

    async fn audio_spans(&self) -> SpanStream<'_, (), AudioData> {
        SpanStream::new(futures::stream::iter(std::iter::once(
            Span::new((), AudioData::new(self.bytes.clone())),
        )))
    }

    async fn edit_audio(
        &mut self,
        edits: SpanEditStream<'_, (), AudioData>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        if let Some(edit) = edits.into_iter().next() {
            self.bytes = edit.data.into_inner();
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl AudioRedact for WavHandler {
    async fn redact_audio(&mut self, _redactions: &[AudioRedaction]) -> Result<(), Error> {
        tracing::warn!("WAV audio redaction is not yet implemented");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::{SpanEdit, AudioHandler};

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
        h.edit_audio(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit::new((), AudioData::new(Bytes::from_static(b"replaced"))),
        ])))
        .await?;
        assert_eq!(h.bytes().as_ref(), b"replaced");
        Ok(())
    }

    #[test]
    fn encode_returns_current_bytes() -> Result<(), Error> {
        let h = WavHandler::new(Bytes::from_static(b"audio-data"));
        let encoded = h.encode()?;
        assert_eq!(&encoded[..], b"audio-data");
        Ok(())
    }
}
