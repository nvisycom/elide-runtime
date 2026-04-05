//! Tabular-modality entity location.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Overlap;

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
    /// The cell text at this location.
    ///
    /// Populated during detection; skipped in serialization to prevent
    /// sensitive data from appearing in API responses.
    #[builder(default)]
    #[serde(default, skip_serializing)]
    pub text: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(row: usize, col: usize) -> TabularLocation {
        TabularLocation::builder()
            .with_row_index(row)
            .with_column_index(col)
            .build()
            .unwrap()
    }

    fn cell_with_offsets(row: usize, col: usize, start: usize, end: usize) -> TabularLocation {
        TabularLocation {
            start_offset: Some(start),
            end_offset: Some(end),
            ..cell(row, col)
        }
    }

    #[test]
    fn builder_required_fields() {
        let loc = cell(1, 2);
        assert_eq!(loc.row_index, 1);
        assert_eq!(loc.column_index, 2);
        assert!(loc.text.is_empty());
    }

    #[test]
    fn overlap_same_cell_no_offsets() {
        assert!(cell(0, 0).overlaps(&cell(0, 0)));
    }

    #[test]
    fn no_overlap_different_row() {
        assert!(!cell(0, 0).overlaps(&cell(1, 0)));
    }

    #[test]
    fn no_overlap_different_col() {
        assert!(!cell(0, 0).overlaps(&cell(0, 1)));
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
        assert!(cell(0, 0).overlaps(&cell_with_offsets(0, 0, 5, 10)));
    }
}
