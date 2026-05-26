//! Tabular modality.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Mergeable, Modality, Overlap};
use crate::document::Span;
use crate::primitive::Confidence;

/// A cell (or sub-cell range) within tabular content.
#[derive(Debug, Clone, PartialEq, Eq, Builder)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "TabularBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct Tabular {
    /// Row index (0-based).
    pub row_index: usize,
    /// Column index (0-based).
    pub column_index: usize,
    /// Byte offset within the cell where the range starts, if applicable.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_offset: Option<usize>,
    /// Byte offset within the cell where the range ends, if applicable.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_offset: Option<usize>,
    /// Column name or header label.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_name: Option<String>,
    /// Sheet or table name (for multi-sheet documents).
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet_name: Option<String>,
}

impl Tabular {
    /// Create a [`Tabular`] for the given cell coordinates, with every
    /// optional field (intra-cell offsets, column name, sheet name)
    /// unset. Use [`builder`] when any of those need to be set.
    ///
    /// [`builder`]: Self::builder
    pub fn new(row_index: usize, column_index: usize) -> Self {
        Self {
            row_index,
            column_index,
            start_offset: None,
            end_offset: None,
            column_name: None,
            sheet_name: None,
        }
    }

    /// Create a new [`TabularBuilder`].
    pub fn builder() -> TabularBuilder {
        TabularBuilder::default()
    }
}

impl Modality for Tabular {
    type Block = TabularBlock;
    type Metadata = TabularMetadata;
    // Tabular shares text strategies — the cell's redacted value is
    // recomputed text-side.
    type Strategy = crate::policy::TextStrategy;
}

/// One row of a tabular document.
///
/// `kind` carries the variant payload (today only [`Row`](TabularBlockKind::Row));
/// `confidence` is the recognition confidence for the row as a whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TabularBlock {
    /// Variant-specific payload.
    #[serde(flatten)]
    pub kind: TabularBlockKind,
    /// Recognition confidence for the row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,
}

/// Variants of [`TabularBlock`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum TabularBlockKind {
    /// A row. Carries the flat row text plus per-cell
    /// [`Span<Tabular>`]s mapping byte offsets back to cell
    /// coordinates.
    Row {
        text: String,
        spans: Vec<Span<Tabular>>,
        /// 0-based row index.
        row_index: usize,
    },
}

impl TabularBlock {
    /// The row's flat text.
    pub fn text(&self) -> &str {
        self.kind.text()
    }

    /// The row's per-cell spans.
    pub fn spans(&self) -> &[Span<Tabular>] {
        self.kind.spans()
    }
}

impl TabularBlockKind {
    pub fn text(&self) -> &str {
        match self {
            Self::Row { text, .. } => text,
        }
    }

    pub fn spans(&self) -> &[Span<Tabular>] {
        match self {
            Self::Row { spans, .. } => spans,
        }
    }
}

/// Document-level metadata for [`Document<Tabular>`].
///
/// [`Document<Tabular>`]: crate::document::Document
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TabularMetadata {
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

impl Overlap for Tabular {
    fn overlaps(&self, other: &Self) -> bool {
        if self.row_index != other.row_index || self.column_index != other.column_index {
            return false;
        }
        match (
            self.start_offset,
            self.end_offset,
            other.start_offset,
            other.end_offset,
        ) {
            (Some(s1), Some(e1), Some(s2), Some(e2)) => s1 < e2 && s2 < e1,
            _ => true,
        }
    }
}

impl Mergeable for Tabular {
    /// Merge two tabular ranges when their cell coordinates match
    /// (same `row_index` + `column_index` + `sheet_name`). Intra-cell
    /// byte offsets union when present on both sides; otherwise the
    /// result has no offsets (meaning "whole cell").
    fn try_merge(self, other: Self) -> Option<Self> {
        if self.row_index != other.row_index
            || self.column_index != other.column_index
            || self.sheet_name != other.sheet_name
        {
            return None;
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
        Some(Self {
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

    fn cell_with_offsets(row: usize, col: usize, start: usize, end: usize) -> Tabular {
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
