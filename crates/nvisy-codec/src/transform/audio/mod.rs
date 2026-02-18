//! Audio redaction output type and rendering primitives.

mod output;

pub use output::AudioRedactionOutput;

use crate::handler::Handler;
use nvisy_core::error::Error;

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

/// Trait for handlers that support audio redaction.
///
/// Extends [`Handler`] with a single [`redact_spans`](Self::redact_spans)
/// method that applies a batch of time-range audio redactions.
#[async_trait::async_trait]
pub trait AudioHandler: Handler {
    /// Apply a batch of audio redactions, mutating in place.
    async fn redact_spans(&mut self, redactions: &[AudioRedaction]) -> Result<(), Error>;
}
