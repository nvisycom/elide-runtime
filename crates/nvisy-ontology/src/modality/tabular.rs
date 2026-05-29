//! Tabular modality.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Mergeable, Modality, ModalityBlock, Overlap, TabularExtraction};
use crate::policy::TabularStrategy;

/// A cell (or sub-cell range) within tabular content.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Tabular {
    /// Row index (0-based).
    pub row_index: u32,
    /// Column index (0-based). Matches the width of
    /// [`ColumnHeader::column_index`] so cell → header joins never
    /// need a cast.
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

impl Tabular {
    /// Create a [`Tabular`] for the given cell coordinates, with every
    /// optional field (intra-cell offsets, column name, sheet name)
    /// unset.
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
}

impl Modality for Tabular {
    type Block = TabularBlock;
    type Extraction = TabularExtraction;
    type Metadata = TabularMetadata;
    type MethodTag = crate::policy::TabularMethodTag;
    type Replacement = crate::provenance::TabularReplacement;
    type Strategy = TabularStrategy;

    fn default_method_dominance() -> &'static [Self::MethodTag] {
        // Clear leaves the cell at known coordinates with an empty
        // value (least content leaks); Mask preserves length;
        // Replace can change length.
        &[
            crate::policy::TabularMethodTag::Clear,
            crate::policy::TabularMethodTag::Mask,
            crate::policy::TabularMethodTag::Replace,
        ]
    }
}

/// Per-modality block payload for [`Tabular`]. Today only
/// [`Row`](Self::Row) — carries the flat row text and its index.
/// Per-cell source spans live on the wrapping [`Block<Tabular>`].
///
/// [`Block<Tabular>`]: crate::document::Block
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TabularBlock {
    /// A row. The row index is carried on each
    /// [`Span<Tabular>::source`] (every span maps a sub-range of
    /// `text` back to its originating cell — the cell's `row_index`
    /// and `column_index` live there), so it's not duplicated at
    /// the block level.
    ///
    /// [`Span<Tabular>::source`]: crate::document::Span::source
    Row { text: String },
}

impl TabularBlock {
    /// The row's flat text.
    pub fn text(&self) -> &str {
        match self {
            Self::Row { text } => text,
        }
    }
}

impl ModalityBlock for TabularBlock {
    fn scan_text(&self) -> Option<&str> {
        Some(self.text())
    }
}

/// Document-level metadata for [`Document<Tabular>`].
///
/// [`Document<Tabular>`]: crate::document::Document
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TabularMetadata {
    /// How this document's tabular structure was produced
    /// (schema-typed from source, inferred, or recovered from an image).
    pub extraction: TabularExtraction,
    /// Column headers indexed by 0-based column position.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<ColumnHeader>,
    /// Sheet names for multi-sheet documents, in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sheet_names: Vec<String>,
}

/// A column header in a tabular document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ColumnHeader {
    /// 0-based column index.
    pub column_index: u32,
    /// Header text.
    pub text: String,
}

impl Tabular {
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

impl Overlap for Tabular {
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

impl Mergeable for Tabular {
    /// Merge two tabular ranges when their cell coordinates match
    /// (same `row_index` + `column_index` + `sheet_name`). Intra-cell
    /// byte offsets union when present on both sides; otherwise the
    /// result has no offsets (meaning "whole cell").
    fn try_merge(self, other: Self) -> Result<Self, (Self, Self)> {
        if self.row_index != other.row_index
            || self.column_index != other.column_index
            || self.sheet_name != other.sheet_name
        {
            return Err((self, other));
        }
        let (start, end) = match (
            self.start_offset,
            self.end_offset,
            other.start_offset,
            other.end_offset,
        ) {
            (Some(s1), Some(e1), Some(s2), Some(e2)) => (Some(s1.min(s2)), Some(e1.max(e2))),
            _ => (None, None),
        };
        Ok(Self {
            row_index: self.row_index,
            column_index: self.column_index,
            start_offset: start,
            end_offset: end,
            column_name: self.column_name.or(other.column_name),
            sheet_name: self.sheet_name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell_with_offsets(row: u32, col: u32, start: usize, end: usize) -> Tabular {
        Tabular {
            start_offset: Some(start),
            end_offset: Some(end),
            ..Tabular::new(row, col)
        }
    }

    #[test]
    fn overlap_same_cell_no_offsets() {
        assert!(Tabular::new(0, 0).overlaps(&Tabular::new(0, 0)));
    }

    #[test]
    fn no_overlap_different_row() {
        assert!(!Tabular::new(0, 0).overlaps(&Tabular::new(1, 0)));
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
        assert!(Tabular::new(0, 0).overlaps(&cell_with_offsets(0, 0, 5, 10)));
    }
}
