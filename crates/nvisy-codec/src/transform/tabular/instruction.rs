//! Tabular redaction instruction types.

use crate::transform::{Mergeable, TextOutput};

/// A tabular redaction targeting a byte range within its containing cell.
///
/// Cell identity is supplied externally via [`Redactions`] — this
/// struct only carries the intra-cell byte range and the replacement
/// output.
///
/// This is the tabular counterpart of [`TextRedaction`]: instead of
/// being grouped by a text span, it is grouped by a [`TabularLocation`]
/// cell.
///
/// [`Redactions`]: crate::transform::Redactions
/// [`TextRedaction`]: crate::transform::TextRedaction
/// [`TabularLocation`]: nvisy_ontology::entity::TabularLocation
#[derive(Debug, Clone, PartialEq)]
pub struct TabularRedaction {
    /// Byte offset where the redacted region starts within the cell value.
    pub start: usize,
    /// Byte offset where the redacted region ends (exclusive) within the cell value.
    pub end: usize,
    /// The redaction output that carries the replacement value.
    pub output: TextOutput,
}

impl TabularRedaction {
    /// Create a new tabular redaction.
    pub fn new(start: usize, end: usize, output: TextOutput) -> Self {
        Self { start, end, output }
    }
}

impl Mergeable for TabularRedaction {
    fn overlaps(&self, other: &Self) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// Merge two overlapping tabular redactions.
    ///
    /// Returns `Some` only when both share the same [`TextOutput`] —
    /// the merged redaction unions the byte ranges. Returns `None`
    /// when the outputs differ.
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
