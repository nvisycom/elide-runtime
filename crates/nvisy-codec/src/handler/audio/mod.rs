//! Audio format handlers and loaders.

use std::cmp::Reverse;

use nvisy_core::Error;
use nvisy_ontology::entity::AudioLocation;

use super::Handler;
use crate::document::LocationStream;
use crate::handler::Redactions;

mod apply;
mod audio_data;
mod audio_handler;
mod instruction;
mod mp3_handler;
mod mp3_loader;
mod wav_handler;
mod wav_loader;

pub(crate) use self::apply::apply_audio_redaction;
pub use self::audio_data::AudioData;
pub use self::instruction::{AudioOutput, AudioRedaction};
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
/// Batched redaction is provided by [`redact`], which overrides the
/// default loop ordering to apply later time spans first — an
/// [`AudioOutput::Remove`] shrinks the buffer and shifts every later
/// sample index, so right-to-left order keeps earlier indices valid.
///
/// [`locations`]: AudioHandler::locations
/// [`read`]: AudioHandler::read
/// [`redact_at`]: AudioHandler::redact_at
/// [`redact`]: AudioHandler::redact
/// [`AudioOutput::Remove`]: crate::handler::AudioOutput::Remove
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

    /// Apply every `(location, redaction)` pair in `redactions` to the
    /// handler, sorted right-to-left by `time_span.start_us`. The first
    /// error aborts the batch.
    async fn redact(
        &mut self,
        redactions: Redactions<AudioLocation, AudioRedaction>,
    ) -> Result<(), Error> {
        let mut items: Vec<_> = redactions.into_iter().collect();
        items.sort_by_key(|(loc, _)| Reverse(loc.time_span.start_us));
        for (location, redaction) in items {
            self.redact_at(&location, redaction).await?;
        }
        Ok(())
    }
}
