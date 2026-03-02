//! [`AnyAudio`]: type-erased wrapper over all audio handler types.

use futures::StreamExt;

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;

use crate::handler::{Handler, AudioHandler, SpanEditStream, SpanStream};
use crate::transform::{AudioRedact, AudioRedaction};

use super::{AudioData, Mp3Handler, WavHandler};

/// A type-erased audio handler that can hold any supported audio format.
///
/// Since all audio handlers share `AudioId = ()`, this enum can
/// implement [`Handler`] + [`AudioHandler`] directly.
#[derive(Debug, Clone, derive_more::From)]
pub enum AnyAudio {
    Wav(WavHandler),
    Mp3(Mp3Handler),
}

impl AnyAudio {
    /// Try to get the inner [`WavHandler`] by reference.
    pub fn as_wav(&self) -> Option<&WavHandler> {
        if let Self::Wav(h) = self { Some(h) } else { None }
    }

    /// Consume and return the inner [`WavHandler`].
    pub fn into_wav(self) -> Option<WavHandler> {
        if let Self::Wav(h) = self { Some(h) } else { None }
    }

    /// Try to get the inner [`Mp3Handler`] by reference.
    pub fn as_mp3(&self) -> Option<&Mp3Handler> {
        if let Self::Mp3(h) = self { Some(h) } else { None }
    }

    /// Consume and return the inner [`Mp3Handler`].
    pub fn into_mp3(self) -> Option<Mp3Handler> {
        if let Self::Mp3(h) = self { Some(h) } else { None }
    }
}

impl Handler for AnyAudio {
    fn document_type(&self) -> DocumentType {
        match self {
            Self::Wav(h) => h.document_type(),
            Self::Mp3(h) => h.document_type(),
        }
    }

    fn encode(&self) -> Result<ContentData, Error> {
        match self {
            Self::Wav(h) => h.encode(),
            Self::Mp3(h) => h.encode(),
        }
    }
}

#[async_trait::async_trait]
impl AudioHandler for AnyAudio {
    type AudioId = ();

    async fn audio_spans(&self) -> SpanStream<'_, (), AudioData> {
        match self {
            Self::Wav(h) => h.audio_spans().await,
            Self::Mp3(h) => h.audio_spans().await,
        }
    }

    async fn edit_audio(
        &mut self,
        edits: SpanEditStream<'_, (), AudioData>,
    ) -> Result<(), Error> {
        // Collect and re-dispatch since we need to forward the stream.
        let edits: Vec<_> = edits.collect().await;
        let stream = SpanEditStream::new(futures::stream::iter(edits));
        match self {
            Self::Wav(h) => h.edit_audio(stream).await,
            Self::Mp3(h) => h.edit_audio(stream).await,
        }
    }
}

#[async_trait::async_trait]
impl AudioRedact for AnyAudio {
    async fn redact_audio(&mut self, redactions: &[AudioRedaction]) -> Result<(), Error> {
        match self {
            Self::Wav(h) => h.redact_audio(redactions).await,
            Self::Mp3(h) => h.redact_audio(redactions).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::AudioHandler;

    #[tokio::test]
    async fn wav_variant_delegates() {
        let h = AnyAudio::Wav(WavHandler::new(bytes::Bytes::from_static(b"wav-data")));
        assert_eq!(h.document_type(), DocumentType::Wav);
        let spans: Vec<_> = h.audio_spans().await.collect().await;
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].data.as_bytes().as_ref(), b"wav-data");
    }

    #[tokio::test]
    async fn mp3_variant_delegates() {
        let h = AnyAudio::Mp3(Mp3Handler::new(bytes::Bytes::from_static(b"mp3-data")));
        assert_eq!(h.document_type(), DocumentType::Mp3);
        assert_eq!(h.encode().unwrap().as_bytes(), b"mp3-data");
    }

    #[test]
    fn from_conversions() {
        let wav: AnyAudio = WavHandler::new(bytes::Bytes::new()).into();
        assert!(wav.as_wav().is_some());
        let mp3: AnyAudio = Mp3Handler::new(bytes::Bytes::new()).into();
        assert!(mp3.as_mp3().is_some());
    }
}
