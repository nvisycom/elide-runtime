//! Audio redaction instruction types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::handler::Mergeable;

/// An audio redaction: the *how*. The *where* (time span, speaker,
/// audio id) lives on the containing [`Audio`] via
/// [`Redactions`]'s `(S, R)` pairs.
///
/// [`Audio`]: nvisy_ontology::modality::Audio
/// [`Redactions`]: crate::handler::Redactions
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

impl Mergeable for AudioRedaction {
    /// Combine two redactions that target overlapping locations.
    /// Returns `Some` only when the outputs match; conflicting methods
    /// (e.g. silence vs remove) cannot be reconciled.
    fn try_merge(self, other: Self) -> Option<Self> {
        (self.output == other.output).then_some(self)
    }
}
