//! Tabular redaction instruction types.

use crate::transform::{Mergeable, TextOutput};

/// A tabular redaction: the *how*. The *where* — cell coordinates
/// (`row_index`, `column_index`) and optional intra-cell byte offsets
/// — lives on the containing [`TabularLocation`] via [`Redactions`]'s
/// `(S, R)` pairs.
///
/// This is the tabular counterpart of [`TextRedaction`]: instead of
/// being grouped by a line-level text span, it is grouped by a cell
/// coordinate.
///
/// [`Redactions`]: crate::transform::Redactions
/// [`TextRedaction`]: crate::transform::TextRedaction
/// [`TabularLocation`]: nvisy_ontology::entity::TabularLocation
#[derive(Debug, Clone, PartialEq)]
pub struct TabularRedaction {
    /// The redaction output that carries the replacement value.
    pub(crate) output: TextOutput,
}

impl TabularRedaction {
    /// Create a new tabular redaction with the given output.
    pub fn new(output: TextOutput) -> Self {
        Self { output }
    }
}

impl Mergeable for TabularRedaction {
    /// Combine two redactions that target overlapping cells. Returns
    /// `Some` only when the outputs match; different replacement
    /// strings cannot be reconciled.
    fn try_merge(self, other: Self) -> Option<Self> {
        (self.output == other.output).then_some(self)
    }
}
