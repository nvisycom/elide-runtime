//! [`AudioTransform`] async trait and blanket implementation.

use nvisy_core::Error;
use nvisy_ontology::entity::AudioLocation;

use super::instruction::AudioRedaction;
use crate::handler::AudioHandler;
use crate::transform::Redactions;

const TARGET: &str = "nvisy_codec::transform::audio";

/// Extension trait for handlers that support audio redaction.
#[async_trait::async_trait]
pub trait AudioTransform: AudioHandler {
    /// Apply a batch of audio redactions, mutating in place.
    ///
    /// Redactions are grouped by [`AudioLocation`] span in the input
    /// [`Redactions`] collection. Time-span overlaps within a span are
    /// resolved by the collection on insert.
    async fn redact_audio(
        &mut self,
        redactions: Redactions<AudioLocation, AudioRedaction>,
    ) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl<H: AudioHandler> AudioTransform for H {
    async fn redact_audio(
        &mut self,
        redactions: Redactions<AudioLocation, AudioRedaction>,
    ) -> Result<(), Error> {
        tracing::debug!(
            target: TARGET,
            redaction_count = redactions.len(),
            "applying audio redactions"
        );
        if redactions.is_empty() {
            return Ok(());
        }

        // TODO: implement audio redaction (silence/remove time ranges)
        tracing::warn!(target: TARGET, "audio redaction is not yet implemented");
        Ok(())
    }
}
