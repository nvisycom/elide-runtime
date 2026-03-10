//! WAV handler: holds raw WAV audio bytes and provides span-based
//! access via [`AudioHandler`](crate::handler::AudioHandler).
//!
//! # Span model
//!
//! [`AudioHandler::audio_spans`] yields a single [`Span`] carrying the
//! entire audio payload as [`AudioData`].  [`AudioHandler::edit_audio`]
//! replaces the payload from the first incoming edit.

use nvisy_core::path::ContentSource;

use super::impl_audio_handler;

/// Handler for loaded WAV content.
///
/// Stores the raw audio bytes directly. The bytes can be produced
/// on demand via [`Handler::encode`](crate::handler::Handler::encode).
#[derive(Debug)]
pub struct WavHandler {
    source: ContentSource,
    bytes: bytes::Bytes,
}

impl_audio_handler!(
    WavHandler,
    nvisy_core::fs::DocumentType::Audio(nvisy_core::fs::AudioFormat::Wav),
    "wav-handler",
    "wav.encode"
);

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use nvisy_core::Error;

    use super::*;
    use crate::document::{Span, SpanStream};
    use crate::handler::{AudioData, AudioHandler, AudioSpanId, Handler};

    #[tokio::test]
    async fn view_spans_returns_single_span() {
        use futures::StreamExt;
        let h = WavHandler::new(Bytes::from_static(b"RIFF-wav-data"));
        let spans: Vec<_> = h.audio_spans().await.collect().await;
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].data.as_bytes().as_ref(), b"RIFF-wav-data");
    }

    #[tokio::test]
    async fn edit_spans_replaces_bytes() -> Result<(), Error> {
        let mut h = WavHandler::new(Bytes::from_static(b"original"));
        h.edit_audio(SpanStream::new(futures::stream::iter(vec![Span::new(
            AudioSpanId::default(),
            AudioData::new(Bytes::from_static(b"replaced")),
        )])))
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
