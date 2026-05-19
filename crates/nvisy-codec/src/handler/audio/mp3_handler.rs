//! MP3 handler: holds raw MP3 audio bytes and provides location-based
//! access via [`AudioHandler`].
//!
//! [`AudioHandler::locations`] yields a single full-duration
//! [`AudioLocation`]; [`AudioHandler::read`] returns the underlying
//! bytes as [`AudioData`]. Redaction is currently a no-op.
//!
//! [`AudioHandler`]: crate::handler::AudioHandler
//! [`AudioHandler::locations`]: crate::handler::AudioHandler::locations
//! [`AudioHandler::read`]: crate::handler::AudioHandler::read
//! [`AudioLocation`]: nvisy_ontology::entity::AudioLocation

use nvisy_core::content::ContentSource;

use super::impl_audio_handler;

/// Handler for loaded MP3 content.
///
/// Stores the raw audio bytes directly. The bytes can be produced
/// on demand via [`Handler::encode`].
///
/// [`Handler::encode`]: crate::handler::Handler::encode
#[derive(Debug)]
pub struct Mp3Handler {
    source: ContentSource,
    bytes: bytes::Bytes,
}

impl_audio_handler!(
    Mp3Handler,
    nvisy_core::media::DocumentType::Audio(nvisy_core::media::AudioFormat::Mp3),
    "mp3-handler",
    "mp3.encode"
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
        let h = Mp3Handler::new(Bytes::from_static(b"ID3-mp3-data"));
        let items: Vec<_> = h.locations().collect().await;
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn read_returns_full_audio() {
        let h = Mp3Handler::new(Bytes::from_static(b"ID3-mp3-data"));
        let items: Vec<_> = h.locations().collect().await;
        let data = h.read(&items[0].location).await.unwrap();
        assert_eq!(data.as_bytes().as_ref(), b"ID3-mp3-data");
    }

    #[test]
    fn encode_returns_current_bytes() -> Result<(), Error> {
        let h = Mp3Handler::new(Bytes::from_static(b"audio-data"));
        let encoded = h.encode()?;
        assert_eq!(encoded.as_bytes(), b"audio-data");
        Ok(())
    }
}
