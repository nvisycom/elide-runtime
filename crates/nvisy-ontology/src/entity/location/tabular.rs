//! Tabular-modality entity location.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Location of an entity within tabular data.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TabularLocation {
    /// Row index (0-based).
    pub row_index: usize,
    /// Column index (0-based).
    pub column_index: usize,
    /// Byte offset within the cell where the entity starts, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_offset: Option<usize>,
    /// Byte offset within the cell where the entity ends, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_offset: Option<usize>,
    /// Column name or header label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_name: Option<String>,
    /// Sheet or table name (for multi-sheet documents).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet_name: Option<String>,
}
