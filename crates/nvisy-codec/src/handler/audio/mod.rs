//! Audio format handlers and loaders.

use nvisy_core::Error;
use nvisy_ontology::entity::AudioLocation;

use super::Handler;
use crate::document::LocationStream;
use crate::transform::AudioRedaction;

mod apply;
mod audio_data;
mod audio_handler;
mod mp3_handler;
mod mp3_loader;
mod wav_handler;
mod wav_loader;

pub(crate) use self::apply::apply_audio_redaction;
pub use self::audio_data::AudioData;
pub use self::audio_handler::BoxedAudioHandler;
pub use self::mp3_handler::Mp3Handler;
pub use self::mp3_loader::{Mp3Loader, Mp3Params};
pub use self::wav_handler::WavHandler;
pub use self::wav_loader::{WavLoader, WavParams};

/// Capability trait for handlers that expose audio content.
///
/// Handlers implement three narrow operations:
/// - [`locations`]: cheap, identity-only stream of [`AudioLocation`]s.
/// - [`read`]: fetch the payload for the time range identified by a
///   location.
/// - [`redact_at`]: apply a single redaction at a single time range.
///
/// Batched redaction lives on the blanket-impl [`AudioTransform::redact`].
///
/// [`locations`]: AudioHandler::locations
/// [`read`]: AudioHandler::read
/// [`redact_at`]: AudioHandler::redact_at
/// [`AudioTransform::redact`]: crate::transform::AudioTransform::redact
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

    /// Apply a single redaction to the time range identified by
    /// `location`, mutating in place.
    async fn redact_at(
        &mut self,
        location: &AudioLocation,
        redaction: AudioRedaction,
    ) -> Result<(), Error>;
}
