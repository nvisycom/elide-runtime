//! WAV handler: holds raw WAV audio bytes and provides location-based
//! access via [`AudioHandler`](crate::handler::AudioHandler).
//!
//! [`AudioHandler::locations`] yields a single full-duration
//! [`AudioLocation`]; [`AudioHandler::read`] returns the underlying
//! bytes as [`AudioData`]. Redaction is currently a no-op.
//!
//! [`AudioHandler::locations`]: crate::handler::AudioHandler::locations
//! [`AudioHandler::read`]: crate::handler::AudioHandler::read
//! [`AudioLocation`]: nvisy_ontology::entity::AudioLocation

use nvisy_core::content::ContentSource;

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
    nvisy_core::media::DocumentType::Audio(nvisy_core::media::AudioFormat::Wav),
    "wav-handler",
    "wav.encode"
);

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use futures::StreamExt;
    use nvisy_core::Error;

    use super::*;
    use crate::handler::{AudioHandler, Handler};

    #[tokio::test]
    async fn locations_yields_single_location() {
        let h = WavHandler::new(Bytes::from_static(b"RIFF-wav-data"));
        let items: Vec<_> = h.locations().collect().await;
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn read_returns_full_audio() {
        let h = WavHandler::new(Bytes::from_static(b"RIFF-wav-data"));
        let items: Vec<_> = h.locations().collect().await;
        let data = h.read(&items[0].location).await.unwrap();
        assert_eq!(data.as_bytes().as_ref(), b"RIFF-wav-data");
    }

    #[test]
    fn encode_returns_current_bytes() -> Result<(), Error> {
        let h = WavHandler::new(Bytes::from_static(b"audio-data"));
        let encoded = h.encode()?;
        assert_eq!(encoded.as_bytes(), b"audio-data");
        Ok(())
    }
}
