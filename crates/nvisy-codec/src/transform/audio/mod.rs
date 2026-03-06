//! Audio redaction output type and rendering primitives.

mod instruction;

use nvisy_core::Error;
pub use instruction::{AudioOutput, AudioRedaction};

use crate::handler::AudioHandler;

/// Extension trait for handlers that support audio redaction.
///
/// Extends [`AudioHandler`] with a single
/// [`redact_audio`](Self::redact_audio) method that applies a batch of
/// time-range audio redactions.  No blanket implementation is provided;
/// each audio handler implements its own logic.
#[async_trait::async_trait]
pub trait AudioRedact: AudioHandler {
    /// Apply a batch of audio redactions, mutating in place.
    async fn redact_audio(
        &mut self,
        redactions: &[AudioRedaction<Self::AudioId>],
    ) -> Result<(), Error>;
}
