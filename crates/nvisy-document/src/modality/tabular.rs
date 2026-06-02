//! Tabular-modality document shape: [`TabularBlock`],
//! [`TabularMetadata`], [`ColumnHeader`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{ModalityBlock, TabularExtraction};

/// Per-modality block payload for [`Tabular`]. Today only [`Row`] —
/// carries the flat row text and its index. Per-cell source spans
/// live on the wrapping [`Block<Tabular>`].
///
/// [`Tabular`]: nvisy_core::modality::Tabular
/// [`Row`]: Self::Row
/// [`Block<Tabular>`]: crate::document::Block
#[derive(Debug, Clone, PartialEq)]
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

impl From<TabularExtraction> for TabularMetadata {
    /// Build [`TabularMetadata`] carrying only the importer-known
    /// extraction tag. Headers and sheet names start empty;
    /// downstream stages fill them in.
    fn from(extraction: TabularExtraction) -> Self {
        Self {
            extraction,
            headers: Vec::new(),
            sheet_names: Vec::new(),
        }
    }
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
