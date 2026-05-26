//! Audio-handler trait + supporting infrastructure.
//!
//! The trait, redaction shape, and `apply_audio_redaction` helper
//! live here; concrete per-format implementations (WAV, MP3) live
//! in `nvisy-formats`.

use std::cmp::Reverse;

use nvisy_core::Error;
use nvisy_ontology::modality::Audio;

use super::Handler;
use crate::document::LocationStream;
use crate::handler::Redactions;

mod apply;
mod audio_data;
mod boxed;
mod instruction;

pub use self::apply::apply_audio_redaction;
pub use self::audio_data::AudioData;
pub use self::boxed::BoxedAudioHandler;
pub use self::instruction::{AudioOutput, AudioRedaction};

/// Capability trait for handlers that expose audio content.
///
/// Handlers implement three narrow operations:
/// - [`locations`]: cheap, identity-only stream of [`Audio`]s.
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
    /// Async stream of [`Audio`]s for this document, each
    /// tagged with the handler's [`ContentSource`].
    ///
    /// [`ContentSource`]: nvisy_core::content::ContentSource
    fn locations(&self) -> LocationStream<'_, Audio>;

    /// Read the audio segment at the given location (time-span slice).
    ///
    /// Returns `None` if the location is out of bounds.
    async fn read(&self, location: &Audio) -> Option<AudioData>;

    /// Apply a single redaction to the time range identified by
    /// `location`, mutating in place.
    async fn redact_at(
        &mut self,
        location: &Audio,
        redaction: AudioRedaction,
    ) -> Result<(), Error>;

    /// Apply every `(location, redaction)` pair in `redactions` to the
    /// handler, sorted right-to-left by `time_span.start_us`. The first
    /// error aborts the batch.
    async fn redact(
        &mut self,
        redactions: Redactions<Audio, AudioRedaction>,
    ) -> Result<(), Error> {
        let mut items: Vec<_> = redactions.into_iter().collect();
        items.sort_by_key(|(loc, _)| Reverse(loc.time_span.start_us));
        for (location, redaction) in items {
            self.redact_at(&location, redaction).await?;
        }
        Ok(())
    }
}
