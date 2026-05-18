//! [`BoxedAudioHandler`]: type-erased wrapper over all audio handler types.

use std::fmt;

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::entity::AudioLocation;

use super::{AudioData, Mp3Handler, WavHandler};
use crate::document::SpanStream;
use crate::handler::{AudioHandler, Handler};

/// A type-erased audio handler backed by a boxed trait object.
pub struct BoxedAudioHandler(Box<dyn AudioHandler>);

impl BoxedAudioHandler {
    /// Wrap any concrete audio handler into a type-erased box.
    pub fn new<H: AudioHandler>(handler: H) -> Self {
        Self(Box::new(handler))
    }
}

impl fmt::Debug for BoxedAudioHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    async fn audio_spans(&self) -> SpanStream<'_, AudioLocation, AudioData> {
        self.0.audio_spans().await
    }

    async fn edit_audio(
        &mut self,
        edits: SpanStream<'_, AudioLocation, AudioData>,
    ) -> Result<(), Error> {
        self.0.edit_audio(edits).await
    }

    async fn value_at(&self, location: &AudioLocation) -> Option<AudioData> {
        self.0.value_at(location).await
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    #[tokio::test]
    async fn wav_variant_delegates() {
        let h = BoxedAudioHandler::from(WavHandler::new(bytes::Bytes::from_static(b"wav-data")));
        assert_eq!(
            h.document_type(),
            DocumentType::Audio(nvisy_core::media::AudioFormat::Wav),
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
            DocumentType::Audio(nvisy_core::media::AudioFormat::Mp3),
        );
        assert_eq!(h.encode().unwrap().as_bytes(), b"mp3-data");
    }
}
