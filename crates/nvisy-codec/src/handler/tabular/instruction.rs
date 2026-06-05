//! Tabular redaction instruction types.

use crate::handler::TextOutput;

/// A tabular redaction: the *how*. The *where* — cell coordinates
/// (`row_index`, `column_index`) and optional intra-cell byte offsets
/// — lives on the containing [`Tabular`] via [`Redactions`]'s
/// `(S, R)` pairs.
///
/// This is the tabular counterpart of [`TextRedaction`]: instead of
/// being grouped by a line-level text span, it is grouped by a cell
/// coordinate.
///
/// [`Redactions`]: crate::core::Redactions
/// [`TextRedaction`]: crate::handler::TextRedaction
/// [`Tabular`]: nvisy_core::modality::Tabular
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

    /// The redaction output that carries the replacement value.
    pub fn output(&self) -> &TextOutput {
        &self.output
    }
}
