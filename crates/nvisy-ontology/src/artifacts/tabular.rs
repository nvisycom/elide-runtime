//! Tabular-modality artifacts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A column header in a tabular document.
///
/// Only columns with detected headers get an entry, so this
/// representation naturally supports gaps (headerless columns).
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ColumnHeader {
    /// 0-based column index.
    pub column_index: u32,
    /// Header text.
    pub text: String,
}

/// Artifacts produced during processing of tabular content.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TabularArtifacts {
    /// Number of data rows (excluding headers).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_count: Option<u32>,
    /// Number of columns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_count: Option<u32>,
    /// Detected column headers, sparse — only columns with headers are present.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<ColumnHeader>,
}
