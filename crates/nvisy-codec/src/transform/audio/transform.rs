//! [`AudioTransform`] async trait and blanket implementation.

use nvisy_core::Error;

use super::instruction::AudioRedaction;
use crate::handler::AudioHandler;

/// Extension trait for handlers that support audio redaction.
///
/// Extends [`AudioHandler`] with [`redact_audio`](Self::redact_audio)
/// which applies a batch of time-range audio redactions.
#[async_trait::async_trait]
pub trait AudioTransform: AudioHandler {
    /// Apply a batch of audio redactions, mutating in place.
    async fn redact_audio(
        &mut self,
        redactions: &[AudioRedaction<Self::AudioId>],
    ) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl<H: AudioHandler> AudioTransform for H {
    async fn redact_audio(
        &mut self,
        redactions: &[AudioRedaction<Self::AudioId>],
    ) -> Result<(), Error> {
        tracing::debug!(
            redaction_count = redactions.len(),
            "applying audio redactions"
        );
        if redactions.is_empty() {
            return Ok(());
        }

        tracing::warn!("audio redaction is not yet implemented");
        Ok(())
    }
}
