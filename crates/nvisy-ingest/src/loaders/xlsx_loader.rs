//! Excel XLSX/XLS file loader using `calamine`.

use serde::Deserialize;
use std::io::Cursor;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::{Document, TabularData};
use nvisy_core::error::{Error, ErrorKind};
use super::{Loader, LoaderOutput};

use calamine::{Reader, open_workbook_auto_from_rs};

/// Typed parameters for [`XlsxLoader`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XlsxLoaderParams {
    /// Maximum number of rows per sheet. `None` means all rows.
    #[serde(default)]
    pub max_rows: Option<usize>,
    /// Specific sheet names to load. Empty means all sheets.
    #[serde(default)]
    pub sheets: Vec<String>,
}

/// Extracts tabular data per sheet from XLSX/XLS files, plus a flattened
/// text document for regex/dictionary scanning.
pub struct XlsxLoader;

#[async_trait::async_trait]
impl Loader for XlsxLoader {
    type Params = XlsxLoaderParams;

    fn id(&self) -> &str {
        "xlsx"
    }

    fn extensions(&self) -> &[&str] {
        &["xlsx", "xls"]
    }

    fn content_types(&self) -> &[&str] {
        &[
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "application/vnd.ms-excel",
        ]
    }

    async fn load(
        &self,
        blob: &Blob,
        params: &Self::Params,
    ) -> Result<Vec<LoaderOutput>, Error> {
        let cursor = Cursor::new(blob.content.to_vec());
        let mut workbook = open_workbook_auto_from_rs(cursor).map_err(|e| {
            Error::new(ErrorKind::Runtime, format!("XLSX open failed: {e}"))
        })?;

        let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
        let mut outputs = Vec::new();
        let mut all_text_parts = Vec::new();

        for sheet_name in &sheet_names {
            if !params.sheets.is_empty()
                && !params.sheets.iter().any(|s| s == sheet_name)
            {
                continue;
            }

            let range = match workbook.worksheet_range(sheet_name) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("Skipping sheet '{}': {}", sheet_name, e);
                    continue;
                }
            };

            let mut rows_iter = range.rows();

            // First row as headers
            let columns: Vec<String> = match rows_iter.next() {
                Some(header_row) => header_row
                    .iter()
                    .map(|c| c.to_string())
                    .collect(),
                None => continue,
            };

            let mut rows = Vec::new();
            for row in rows_iter {
                if let Some(max) = params.max_rows {
                    if rows.len() >= max {
                        break;
                    }
                }
                let row_data: Vec<String> = row.iter().map(|c| c.to_string()).collect();
                all_text_parts.push(row_data.join("\t"));
                rows.push(row_data);
            }

            let tabular = TabularData::new(columns, rows)
                .with_source_format("xlsx")
                .with_sheet_name(sheet_name);

            outputs.push(LoaderOutput::Tabular(tabular));
        }

        // Create a flattened document for regex/dictionary scanning
        if !all_text_parts.is_empty() {
            let doc = Document::new(all_text_parts.join("\n"))
                .with_source_format("xlsx");
            outputs.push(LoaderOutput::Document(doc));
        }

        Ok(outputs)
    }
}
