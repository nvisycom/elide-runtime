//! Audio redaction instruction types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An audio redaction: the *how*. The *where* (time span, speaker,
/// audio id) lives on the containing [`Audio`] via
/// [`Redactions`]'s `(S, R)` pairs.
///
/// [`Audio`]: nvisy_ontology::modality::Audio
/// [`Redactions`]: crate::core::Redactions
#[derive(Debug, Clone, PartialEq)]
pub struct AudioRedaction {
    /// The redaction output that determines the rendering method.
    pub(crate) output: AudioOutput,
}

impl AudioRedaction {
    /// Create a new audio redaction.
    pub fn new(output: AudioOutput) -> Self {
        Self { output }
    }

    /// The redaction output that determines the rendering method.
    pub fn output(&self) -> &AudioOutput {
        &self.output
    }
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
