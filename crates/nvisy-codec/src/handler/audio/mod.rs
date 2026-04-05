//! Audio format handlers and loaders.

use nvisy_core::Error;
use nvisy_ontology::entity::AudioLocation;

use super::Handler;
use crate::document::SpanStream;

mod audio_data;
mod audio_handler;
mod audio_handler_macro;
mod mp3_handler;
mod mp3_loader;
mod wav_handler;
mod wav_loader;

pub use self::audio_data::AudioData;
pub use self::audio_handler::BoxedAudioHandler;
use self::audio_handler_macro::impl_audio_handler;
pub use self::mp3_handler::Mp3Handler;
pub use self::mp3_loader::{Mp3Loader, Mp3Params};
pub use self::wav_handler::WavHandler;
pub use self::wav_loader::{WavLoader, WavParams};

/// Capability trait for handlers that expose audio content.
///
/// All audio handlers use [`AudioLocation`] as their span identifier.
#[async_trait::async_trait]
pub trait AudioHandler: Handler {
    /// Return audio content as an async stream of [`Span`](crate::document::Span)s.
    ///
    /// Each span carries an [`AudioLocation`] and [`AudioData`] payload.
    async fn audio_spans(&self) -> SpanStream<'_, AudioLocation, AudioData>;

    /// Apply audio edits from an async stream back to the handler.
    async fn edit_audio(
        &mut self,
        edits: SpanStream<'_, AudioLocation, AudioData>,
    ) -> Result<(), Error>;

    /// Extract the audio data at the given location (time span segment).
    ///
    /// Returns `None` if the location is out of bounds.
    async fn value_at(&self, location: &AudioLocation) -> Option<AudioData>;
}
