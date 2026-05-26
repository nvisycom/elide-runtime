//! Tabular-modality entity location.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Mergeable, Overlap};

/// Location of an entity within tabular data.
#[derive(Debug, Clone, PartialEq, Eq, Builder)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "TabularLocationBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct TabularLocation {
    /// Row index (0-based).
    pub row_index: usize,
    /// Column index (0-based).
    pub column_index: usize,
    /// Byte offset within the cell where the entity starts, if applicable.
    #[builder(default, setter(into = false))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_offset: Option<usize>,
    /// Byte offset within the cell where the entity ends, if applicable.
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

impl TabularLocation {
    /// Create a [`TabularLocation`] for the given cell coordinates,
    /// with every optional field (intra-cell offsets, column name,
    /// sheet name) unset. Use [`builder`] when any of those need to
    /// be set.
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

    /// Create a new [`TabularLocationBuilder`].
    pub fn builder() -> TabularLocationBuilder {
        TabularLocationBuilder::default()
    }
}

impl Overlap for TabularLocation {
    fn overlaps(&self, other: &Self) -> bool {
        if self.row_index != other.row_index || self.column_index != other.column_index {
            return false;
        }
        // Same cell — check intra-cell byte ranges if both are present.
        match (
            self.start_offset,
            self.end_offset,
            other.start_offset,
            other.end_offset,
        ) {
            (Some(s1), Some(e1), Some(s2), Some(e2)) => s1 < e2 && s2 < e1,
            _ => true, // no offset info → assume full-cell overlap
        }
    }
}

impl Mergeable for TabularLocation {
    /// Merge two tabular locations when their cell coordinates match
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

    fn cell_with_offsets(row: usize, col: usize, start: usize, end: usize) -> TabularLocation {
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
        // One has offsets, one doesn't → assume overlap.
        assert!(TabularLocation::new(0, 0).overlaps(&cell_with_offsets(0, 0, 5, 10)));
    }
}
