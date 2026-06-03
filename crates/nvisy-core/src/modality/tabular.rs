//! [`Tabular`] modality marker, [`TabularLocation`] coordinate type,
//! and the [`TabularExtraction`] provenance enum.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Modality, Overlap};
use crate::entity::ModelProvenance;

/// Tabular modality marker (zero-sized).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Tabular;

impl Modality for Tabular {
    type Location = TabularLocation;
}

/// A cell (or sub-cell range) within tabular content.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TabularLocation {
    /// Row index (0-based).
    pub row_index: u32,
    /// Column index (0-based).
    pub column_index: u32,
    /// Byte offset within the cell where the range starts, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_offset: Option<usize>,
    /// Byte offset within the cell where the range ends, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_offset: Option<usize>,
    /// Column name or header label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_name: Option<String>,
    /// Sheet or table name (for multi-sheet documents).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet_name: Option<String>,
}

impl TabularLocation {
    /// Create a [`TabularLocation`] for the given cell coordinates,
    /// with every optional field (intra-cell offsets, column name,
    /// sheet name) unset.
    pub fn new(row_index: u32, column_index: u32) -> Self {
        Self {
            row_index,
            column_index,
            start_offset: None,
            end_offset: None,
            column_name: None,
            sheet_name: None,
        }
    }

    /// Inclusive byte range within the cell, treating absent offsets
    /// as "whole cell". Returns `(start, end)` with `start == 0`
    /// when no `start_offset` is set and `end == usize::MAX` when
    /// no `end_offset` is set — a sentinel range that overlaps any
    /// concrete subrange and merges into anything.
    fn cell_range(&self) -> (usize, usize) {
        (
            self.start_offset.unwrap_or(0),
            self.end_offset.unwrap_or(usize::MAX),
        )
    }
}

impl Overlap for TabularLocation {
    /// Two tabular ranges overlap only when they target the same
    /// cell — matching `row_index`, `column_index`, **and**
    /// `sheet_name` — and their intra-cell byte ranges intersect.
    /// Without the sheet gate, two cells at the same row/col across
    /// sheets of a workbook would false-positive as overlapping.
    fn overlaps(&self, other: &Self) -> bool {
        if self.row_index != other.row_index
            || self.column_index != other.column_index
            || self.sheet_name != other.sheet_name
        {
            return false;
        }
        let (s1, e1) = self.cell_range();
        let (s2, e2) = other.cell_range();
        s1 < e2 && s2 < e1
    }
}

/// How a [`Document<Tabular>`]'s structure was produced.
///
/// [`Document<Tabular>`]: # "carrier owned by nvisy-document"
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TabularExtraction {
    /// Header names and column types known from the source format
    /// (Parquet, Avro, CSV with a header row).
    SchemaTyped,
    /// Column semantics inferred from column data (header-less CSV,
    /// positional conventions).
    SchemaInferred,
    /// Tabular structure recovered from an image (row/column
    /// reconstruction from a scanned table); preserves cell
    /// relationships that plain OCR loses.
    Recovered(ModelProvenance),
}

impl TabularExtraction {
    /// Map the codec's `has_header()` signal to the matching
    /// extraction variant.
    ///
    /// `None` means the codec couldn't decide (non-tabular handle
    /// somehow reached this site) — fall back to `SchemaInferred`
    /// rather than panicking, since over-reporting "inferred" is safe.
    pub fn from_header_signal(has_header: Option<bool>) -> Self {
        match has_header {
            Some(true) => Self::SchemaTyped,
            Some(false) | None => Self::SchemaInferred,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell_with_offsets(row: u32, col: u32, start: usize, end: usize) -> TabularLocation {
        TabularLocation {
            start_offset: Some(start),
            end_offset: Some(end),
            ..TabularLocation::new(row, col)
        }
    }

    #[test]
    fn overlap_same_cell_no_offsets() {
        assert!(TabularLocation::new(0, 0).overlaps(&TabularLocation::new(0, 0)));
    }

    #[test]
    fn no_overlap_different_row() {
        assert!(!TabularLocation::new(0, 0).overlaps(&TabularLocation::new(1, 0)));
    }

    #[test]
    fn overlap_same_cell_intersecting_offsets() {
        assert!(cell_with_offsets(0, 0, 0, 10).overlaps(&cell_with_offsets(0, 0, 5, 15)));
    }

    #[test]
    fn no_overlap_same_cell_disjoint_offsets() {
        assert!(!cell_with_offsets(0, 0, 0, 5).overlaps(&cell_with_offsets(0, 0, 5, 10)));
    }

    #[test]
    fn overlap_same_cell_one_has_offsets() {
        assert!(TabularLocation::new(0, 0).overlaps(&cell_with_offsets(0, 0, 5, 10)));
    }
}
