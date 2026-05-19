//! Text redaction instruction types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::transform::Mergeable;

/// A text redaction targeting a byte range within its containing span.
///
/// Span identity is supplied externally via [`Redactions`] — this
/// struct only carries the intra-span byte range and the replacement
/// output.
///
/// [`Redactions`]: crate::transform::Redactions
#[derive(Debug, Clone, PartialEq)]
pub struct TextRedaction {
    /// Byte offset where the redacted region starts within the span.
    pub(crate) start: usize,
    /// Byte offset where the redacted region ends (exclusive) within the span.
    pub(crate) end: usize,
    /// The redaction output that carries the replacement value.
    pub(crate) output: TextOutput,
}

impl TextRedaction {
    /// Create a new text redaction.
    pub fn new(start: usize, end: usize, output: TextOutput) -> Self {
        Self { start, end, output }
    }
}

/// Text redaction output — the codec only needs to know the replacement string
/// or that the span should be removed entirely.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum TextOutput {
    /// Substituted with a replacement string.
    Replace { replacement: String },
    /// Removed entirely from the output.
    Remove,
}

impl TextOutput {
    /// Create a [`Replace`] output with the given string.
    ///
    /// [`Replace`]: Self::Replace
    pub fn replace(replacement: impl Into<String>) -> Self {
        Self::Replace {
            replacement: replacement.into(),
        }
    }

    /// Returns the text replacement string, regardless of specific method.
    ///
    /// Returns `None` for [`Remove`] — the caller should treat that as
    /// an empty string (span deleted).
    ///
    /// [`Remove`]: Self::Remove
    pub fn replacement_value(&self) -> Option<&str> {
        match self {
            Self::Replace { replacement } => Some(replacement),
            Self::Remove => None,
        }
    }
}

impl Mergeable for TextRedaction {
    fn overlaps(&self, other: &Self) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// Merge two overlapping text redactions.
    ///
    /// Returns `Some` only when both share the same [`TextOutput`] —
    /// the merged redaction unions the byte ranges. Returns `None`
    /// when the outputs differ (e.g. `Replace { "[A]" }` vs `Replace { "[B]" }`),
    /// since picking one would silently drop a redaction.
    fn try_merge(self, other: Self) -> Option<Self> {
        if self.output != other.output {
            return None;
        }
        Some(Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            output: self.output,
        })
    }
}
