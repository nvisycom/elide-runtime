//! [`BoxedAudioHandler`]: type-erased wrapper over all audio handler types.

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;

use super::{AudioData, AudioSpanId, Mp3Handler, WavHandler};
use crate::document::SpanStream;
use crate::handler::{AudioHandler, Handler};

/// A type-erased audio handler backed by a boxed trait object.
///
/// All audio handlers share `AudioId = AudioSpanId`, so a single
/// boxed trait object can unify them without per-variant boilerplate.
pub struct BoxedAudioHandler(Box<dyn DynAudioHandler>);

impl BoxedAudioHandler {
    /// Wrap any concrete audio handler into a type-erased box.
    fn new<H: DynAudioHandler>(handler: H) -> Self {
        Self(Box::new(handler))
    }
}

impl std::fmt::Debug for BoxedAudioHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("BoxedAudioHandler")
            .field(&self.0.document_type())
            .finish()
    }
}

impl From<WavHandler> for BoxedAudioHandler {
    fn from(h: WavHandler) -> Self {
        Self::new(h)
    }
}

impl From<Mp3Handler> for BoxedAudioHandler {
    fn from(h: Mp3Handler) -> Self {
        Self::new(h)
    }
}

impl Handler for BoxedAudioHandler {
    fn document_type(&self) -> DocumentType {
        Handler::document_type(self.0.as_ref())
    }

    fn source(&self) -> ContentSource {
        Handler::source(self.0.as_ref())
    }

    fn encode(&self) -> Result<ContentData, Error> {
        Handler::encode(self.0.as_ref())
    }
}

#[async_trait::async_trait]
impl AudioHandler for BoxedAudioHandler {
    type AudioId = AudioSpanId;

    async fn audio_spans(&self) -> SpanStream<'_, AudioSpanId, AudioData> {
        self.0.audio_spans().await
    }

    async fn edit_audio(
        &mut self,
        edits: SpanStream<'_, AudioSpanId, AudioData>,
    ) -> Result<(), Error> {
        self.0.edit_audio(edits).await
    }
}

/// Object-safe supertrait combining Handler + AudioHandler for boxing.
#[async_trait::async_trait]
trait DynAudioHandler: Handler {
    async fn audio_spans(&self) -> SpanStream<'_, AudioSpanId, AudioData>;
    async fn edit_audio(
        &mut self,
        edits: SpanStream<'_, AudioSpanId, AudioData>,
    ) -> Result<(), Error>;
}

macro_rules! impl_dyn_audio {
    ($ty:ty) => {
        #[async_trait::async_trait]
        impl DynAudioHandler for $ty {
            async fn audio_spans(&self) -> SpanStream<'_, AudioSpanId, AudioData> {
                AudioHandler::audio_spans(self).await
            }

            async fn edit_audio(
                &mut self,
                edits: SpanStream<'_, AudioSpanId, AudioData>,
            ) -> Result<(), Error> {
                AudioHandler::edit_audio(self, edits).await
            }
        }
    };
}

impl_dyn_audio!(WavHandler);
impl_dyn_audio!(Mp3Handler);

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    #[tokio::test]
    async fn wav_variant_delegates() {
        let h = BoxedAudioHandler::from(WavHandler::new(bytes::Bytes::from_static(b"wav-data")));
        assert_eq!(
            h.document_type(),
            DocumentType::Audio(nvisy_core::fs::AudioFormat::Wav),
        );
        let spans: Vec<_> = h.audio_spans().await.collect().await;
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].data.as_bytes().as_ref(), b"wav-data");
    }

    #[tokio::test]
    async fn mp3_variant_delegates() {
        let h = BoxedAudioHandler::from(Mp3Handler::new(bytes::Bytes::from_static(b"mp3-data")));
        assert_eq!(
            h.document_type(),
            DocumentType::Audio(nvisy_core::fs::AudioFormat::Mp3),
        );
        assert_eq!(h.encode().unwrap().as_bytes(), b"mp3-data");
    }

    #[test]
    fn from_conversions() {
        let wav = BoxedAudioHandler::from(WavHandler::new(bytes::Bytes::new()));
        assert_eq!(
            wav.document_type(),
            DocumentType::Audio(nvisy_core::fs::AudioFormat::Wav),
        );
        let mp3 = BoxedAudioHandler::from(Mp3Handler::new(bytes::Bytes::new()));
        assert_eq!(
            mp3.document_type(),
            DocumentType::Audio(nvisy_core::fs::AudioFormat::Mp3),
        );
    }
}
