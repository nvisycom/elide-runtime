//! Tabular redaction instruction types.

use nvisy_ontology::entity::TabularLocation;

use super::super::TextOutput;

/// A located tabular redaction: pairs a [`TabularLocation`] (row/col
/// cell address) with intra-cell byte offsets and a [`TextOutput`]
/// that carries the replacement.
///
/// This is the tabular counterpart of [`TextRedaction`]: instead of
/// identifying a span by byte offsets into the serialized form, it
/// addresses a cell by `(row_index, column_index)` and specifies
/// where within that cell's content the redaction applies.
///
/// [`TextRedaction`]: crate::transform::TextRedaction
pub struct TabularRedaction {
    /// Which cell this redaction targets.
    pub location: TabularLocation,
    /// Byte offset where the redacted region starts within the cell value.
    pub start: usize,
    /// Byte offset where the redacted region ends (exclusive) within the cell value.
    pub end: usize,
    /// The redaction output that carries the replacement value.
    pub output: TextOutput,
}
