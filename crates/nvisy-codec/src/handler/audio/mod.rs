//! Audio format handlers and loaders.

use nvisy_core::Error;
use nvisy_ontology::entity::AudioLocation;

use super::Handler;
use crate::document::LocationStream;
use crate::transform::{AudioRedaction, Redactions};

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
/// Handlers expose audio content as a stream of [`AudioLocation`]s
/// (cheap, identity-only), with explicit `read` calls to fetch the
/// payload for any given location, and a `redact` call that applies a
/// batch of [`AudioRedaction`]s grouped by location.
#[async_trait::async_trait]
pub trait AudioHandler: Handler {
    /// Async stream of [`AudioLocation`]s for this document, each
    /// tagged with the handler's [`ContentSource`].
    ///
    /// [`ContentSource`]: nvisy_core::content::ContentSource
    fn locations(&self) -> LocationStream<'_, AudioLocation>;

    /// Read the audio segment at the given location (time-span slice).
    ///
    /// Returns `None` if the location is out of bounds.
    async fn read(&self, location: &AudioLocation) -> Option<AudioData>;

    /// Apply a batch of redactions grouped by [`AudioLocation`].
    async fn redact(
        &mut self,
        redactions: Redactions<AudioLocation, AudioRedaction>,
    ) -> Result<(), Error>;
}
