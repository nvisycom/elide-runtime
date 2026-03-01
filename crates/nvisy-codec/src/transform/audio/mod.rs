//! Audio redaction output type and rendering primitives.

mod output;

pub use output::AudioRedactionOutput;

use crate::handler::AudioHandler;
use nvisy_core::Error;

/// A located audio redaction: pairs a time range with an
/// [`AudioRedactionOutput`] that carries the method-specific parameters.
pub struct AudioRedaction {
    /// Start of the redacted segment in seconds.
    pub start_secs: f64,
    /// End of the redacted segment in seconds.
    pub end_secs: f64,
    /// The redaction output that determines the rendering method.
    pub output: AudioRedactionOutput,
}

/// Extension trait for handlers that support audio redaction.
///
/// Extends [`AudioHandler`] with a single
/// [`redact_audio`](Self::redact_audio) method that applies a batch of
/// time-range audio redactions.  No blanket implementation is provided;
/// each audio handler implements its own logic.
#[async_trait::async_trait]
pub trait AudioRedact: AudioHandler {
    /// Apply a batch of audio redactions, mutating in place.
    async fn redact_audio(&mut self, redactions: &[AudioRedaction]) -> Result<(), Error>;
}
