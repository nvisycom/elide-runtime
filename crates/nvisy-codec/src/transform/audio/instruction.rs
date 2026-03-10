//! Audio redaction instruction types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A located audio redaction: pairs a time range with
/// an [`AudioOutput`] that carries the method-specific parameters.
pub struct AudioRedaction {
    /// Start of the redacted segment in seconds.
    pub start_secs: f64,
    /// End of the redacted segment in seconds.
    pub end_secs: f64,
    /// The redaction output that determines the rendering method.
    pub output: AudioOutput,
}

/// Audio redaction output — records the method used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AudioOutput {
    /// Segment replaced with silence.
    Silence,
    /// Segment removed entirely.
    Remove,
    /// Segment replaced with provided audio data.
    Replace { data: Vec<u8> },
}
