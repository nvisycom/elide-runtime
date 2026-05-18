//! Audio redaction instruction types.

use nvisy_ontology::primitive::TimeSpan;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::transform::Mergeable;

/// An audio redaction targeting a time range within its containing span.
///
/// Span identity is supplied externally via [`Redactions`] — this
/// struct only carries the time span and the rendering method.
///
/// [`Redactions`]: crate::transform::Redactions
#[derive(Debug, Clone, PartialEq)]
pub struct AudioRedaction {
    /// Time interval of the segment to redact.
    pub(crate) time_span: TimeSpan,
    /// The redaction output that determines the rendering method.
    pub(crate) output: AudioOutput,
}

impl AudioRedaction {
    /// Create a new audio redaction.
    pub fn new(time_span: TimeSpan, output: AudioOutput) -> Self {
        Self { time_span, output }
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
    fn overlaps(&self, other: &Self) -> bool {
        self.time_span.overlaps(&other.time_span)
    }

    /// Merge two overlapping audio redactions.
    ///
    /// Returns `Some` only when both share the same [`AudioOutput`]
    /// (method *and* parameters) — the merged redaction unions the
    /// time spans. Returns `None` when the methods differ.
    fn try_merge(self, other: Self) -> Option<Self> {
        if self.output != other.output {
            return None;
        }
        Some(Self {
            time_span: self.time_span.union(&other.time_span),
            output: self.output,
        })
    }
}
