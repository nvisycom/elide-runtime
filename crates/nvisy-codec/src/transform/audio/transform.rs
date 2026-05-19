//! [`AudioTransform`]: blanket-impl extension that iterates a
//! [`Redactions`] collection per-location, dispatching to the
//! handler's [`AudioHandler::redact_at`] hook.

use std::cmp::Reverse;

use nvisy_core::Error;
use nvisy_ontology::entity::AudioLocation;

use super::instruction::AudioRedaction;
use crate::handler::AudioHandler;
use crate::transform::Redactions;

/// Apply a batch of audio redactions, one per location.
///
/// Blanket-implemented for every [`AudioHandler`]: the handler only
/// implements the narrow [`AudioHandler::redact_at`] hook; iteration
/// over the [`Redactions`] collection lives here.
///
/// **Iteration order** is right-to-left by `time_span.start_us` — an
/// [`AudioOutput::Remove`] shrinks the buffer and shifts every
/// later sample-index, so later redactions must apply first to keep
/// earlier ones' indices valid.
///
/// [`AudioOutput::Remove`]: crate::transform::AudioOutput::Remove
#[async_trait::async_trait]
pub trait AudioTransform: AudioHandler {
    /// Apply every `(location, redaction)` pair in `redactions` to
    /// the handler, sorted right-to-left by `time_span.start_us`. The
    /// first error aborts the batch.
    async fn redact(
        &mut self,
        redactions: Redactions<AudioLocation, AudioRedaction>,
    ) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl<H: AudioHandler + ?Sized> AudioTransform for H {
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
