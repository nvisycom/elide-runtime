//! WAV handler: holds raw WAV audio bytes and provides location-based
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

/// Handler for loaded WAV content.
///
/// Stores the raw audio bytes directly. The bytes can be produced
/// on demand via [`Handler::encode`].
///
/// [`Handler::encode`]: crate::handler::Handler::encode
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
