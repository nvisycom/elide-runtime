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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_headers_skip_gaps() {
        // Columns 0 and 3 have headers; columns 1 and 2 do not.
        let artifacts = TabularArtifacts {
            row_count: Some(100),
            column_count: Some(4),
            headers: vec![
                ColumnHeader {
                    column_index: 0,
                    text: "id".to_owned(),
                },
                ColumnHeader {
                    column_index: 3,
                    text: "name".to_owned(),
                },
            ],
        };
        let json = serde_json::to_string(&artifacts).unwrap();
        let back: TabularArtifacts = serde_json::from_str(&json).unwrap();
        assert_eq!(back.headers.len(), 2);
        assert_eq!(back.headers[0].column_index, 0);
        assert_eq!(back.headers[1].column_index, 3);
    }

    #[test]
    fn default_is_empty() {
        let artifacts = TabularArtifacts::default();
        let json = serde_json::to_string(&artifacts).unwrap();
        // All fields skipped — JSON should be empty object.
        assert_eq!(json, "{}");
    }
}
