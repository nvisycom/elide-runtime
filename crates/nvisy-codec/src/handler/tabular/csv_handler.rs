//! CSV handler: holds parsed CSV content and provides cell-coordinate
//! access via [`Handler`] + [`TabularHandler`].
//!
//! [`TabularHandler::locations`] yields one [`TabularLocation`] per
//! cell using `(row, col)` coordinates. Row `0` is the header row
//! (if present); row `1` is the first data row when headers exist,
//! else row `0` is the first data row. [`TabularHandler::read`]
//! returns the cell's value as [`TextData`].
//! [`TabularHandler::redact`] mutates cells by coordinate, applying
//! intra-cell byte-offset replacements via [`apply_tabular_redaction`].
//!
//! [`TabularLocation`]: nvisy_ontology::entity::TabularLocation

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::{DocumentType, SpreadsheetFormat};
use nvisy_ontology::entity::TabularLocation;

use crate::document::{Located, LocationStream};
use crate::handler::text::TextData;
use crate::handler::{Handler, TabularHandler};
use super::{TabularRedaction, apply_tabular_redaction};

const TARGET: &str = "csv-handler";

/// Parsed CSV content.
#[derive(Debug, Clone)]
pub struct CsvData {
    /// Column headers, if present.
    pub headers: Option<Vec<String>>,
    /// Data rows (excluding the header row).
    pub rows: Vec<Vec<String>>,
    /// Field delimiter byte (e.g. `b','`, `b'\t'`, `b';'`).
    pub delimiter: u8,
    /// Whether the original source had a trailing newline.
    pub trailing_newline: bool,
}

/// Handler for loaded CSV content.
#[derive(Debug)]
pub struct CsvHandler {
    source: ContentSource,
    data: CsvData,
}

impl Handler for CsvHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Spreadsheet(SpreadsheetFormat::Csv)
    }

    fn source(&self) -> ContentSource {
        self.source
    }

    #[tracing::instrument(name = "csv.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<ContentData, Error> {
        let bytes = self.serialize_bytes()?;
        tracing::Span::current().record("output_bytes", bytes.len());
        let source = ContentSource::new().with_parent(&self.source);
        Ok(ContentData::new(source, bytes.into()))
    }
}

#[async_trait::async_trait]
impl TabularHandler for CsvHandler {
    fn locations(&self) -> LocationStream<'_, TabularLocation> {
        let source = self.source;
        let has_headers = self.data.headers.is_some();

        let mut items: Vec<_> = Vec::new();

        if let Some(headers) = &self.data.headers {
            for (col, _) in headers.iter().enumerate() {
                items.push(Located::new(
                    source,
                    TabularLocation {
                        row_index: 0,
                        column_index: col,
                        start_offset: None,
                        end_offset: None,
                        column_name: Some(headers[col].clone()),
                        sheet_name: None,
                    },
                ));
            }
        }

        for (data_row, row) in self.data.rows.iter().enumerate() {
            let row_index = if has_headers { data_row + 1 } else { data_row };
            for (col, _) in row.iter().enumerate() {
                items.push(Located::new(
                    source,
                    TabularLocation {
                        row_index,
                        column_index: col,
                        start_offset: None,
                        end_offset: None,
                        column_name: self.data.headers.as_ref().and_then(|h| h.get(col).cloned()),
                        sheet_name: None,
                    },
                ));
            }
        }

        LocationStream::new(futures::stream::iter(items))
    }

    async fn read(&self, location: &TabularLocation) -> Option<TextData> {
        let (is_header, data_row) = self.resolve_row(location.row_index)?;
        let cell = if is_header {
            self.data.headers.as_ref()?.get(location.column_index)?
        } else {
            self.data.rows.get(data_row)?.get(location.column_index)?
        };
        Some(TextData::from(cell.clone()))
    }

    async fn redact_at(
        &mut self,
        location: &TabularLocation,
        redaction: TabularRedaction,
    ) -> Result<(), Error> {
        let Some((is_header, data_row)) = self.resolve_row(location.row_index) else {
            return Ok(());
        };
        let cell = if is_header {
            let Some(headers) = self.data.headers.as_mut() else {
                return Ok(());
            };
            let Some(cell) = headers.get_mut(location.column_index) else {
                return Ok(());
            };
            cell
        } else {
            let Some(row) = self.data.rows.get_mut(data_row) else {
                return Ok(());
            };
            let Some(cell) = row.get_mut(location.column_index) else {
                return Ok(());
            };
            cell
        };
        // Intra-cell byte range comes from the location; omitted means
        // redact the whole cell.
        let start = location.start_offset.unwrap_or(0);
        let end = location.end_offset.unwrap_or(cell.len());
        apply_tabular_redaction(cell, &redaction, start, end, TARGET)
    }
}

impl CsvHandler {
    /// Create a new handler from parsed CSV data.
    pub fn new(data: CsvData) -> Self {
        Self {
            source: ContentSource::new(),
            data,
        }
    }

    /// Set the content source for lineage tracking.
    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }

    /// Column headers, if present.
    pub fn headers(&self) -> Option<&[String]> {
        self.data.headers.as_deref()
    }

    /// All data rows.
    pub fn rows(&self) -> &[Vec<String>] {
        &self.data.rows
    }

    /// Mutable access to all data rows.
    pub fn rows_mut(&mut self) -> &mut Vec<Vec<String>> {
        &mut self.data.rows
    }

    /// A specific cell by `(data_row, col)`.
    ///
    /// `data_row` is 0-based against the data rows (header is *not*
    /// data row 0). Use [`TabularLocation`] coordinates with
    /// [`TabularHandler::read`] if you need to address the header row.
    ///
    /// [`TabularHandler::read`]: crate::handler::TabularHandler::read
    pub fn cell(&self, data_row: usize, col: usize) -> Option<&str> {
        self.data
            .rows
            .get(data_row)
            .and_then(|r| r.get(col))
            .map(|s| s.as_str())
    }

    /// Number of data rows (excluding the header).
    pub fn len(&self) -> usize {
        self.data.rows.len()
    }

    /// Whether the document has no data rows.
    pub fn is_empty(&self) -> bool {
        self.data.rows.is_empty()
    }

    /// Detected field delimiter.
    pub fn delimiter(&self) -> u8 {
        self.data.delimiter
    }

    /// Whether the original source had a trailing newline.
    pub fn trailing_newline(&self) -> bool {
        self.data.trailing_newline
    }

    /// Consume the handler and return the inner [`CsvData`].
    pub fn into_data(self) -> CsvData {
        self.data
    }

    /// Resolve a [`TabularLocation::row_index`] to `(is_header, data_row)`.
    ///
    /// Returns `None` when the row index is out of range.
    ///
    /// [`TabularLocation::row_index`]: nvisy_ontology::entity::TabularLocation::row_index
    fn resolve_row(&self, row_index: usize) -> Option<(bool, usize)> {
        if self.data.headers.is_some() {
            if row_index == 0 {
                Some((true, 0))
            } else {
                let data_row = row_index - 1;
                (data_row < self.data.rows.len()).then_some((false, data_row))
            }
        } else {
            (row_index < self.data.rows.len()).then_some((false, row_index))
        }
    }

    /// Serialize to bytes (with CRLF→LF normalization and trailing
    /// newline handling).
    fn serialize_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut wtr = csv::WriterBuilder::new()
            .delimiter(self.data.delimiter)
            .has_headers(false)
            .from_writer(Vec::new());

        if let Some(headers) = &self.data.headers {
            wtr.write_record(headers)
                .map_err(|e| Error::validation(format!("CSV encode error: {e}"), TARGET))?;
        }
        for row in &self.data.rows {
            wtr.write_record(row)
                .map_err(|e| Error::validation(format!("CSV encode error: {e}"), TARGET))?;
        }
        let mut bytes = wtr
            .into_inner()
            .map_err(|e| Error::validation(format!("CSV encode error: {e}"), TARGET))?;

        bytes.retain(|&b| b != b'\r');

        if !self.data.trailing_newline && bytes.last() == Some(&b'\n') {
            bytes.pop();
        }

        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use nvisy_core::Error;

    use super::*;
    use crate::handler::TabularHandler;
    use crate::handler::{ConflictPolicy, Redactions, TextOutput};

    fn handler_with_headers(headers: Vec<&str>, rows: Vec<Vec<&str>>) -> CsvHandler {
        CsvHandler::new(CsvData {
            headers: Some(headers.into_iter().map(String::from).collect()),
            rows: rows
                .into_iter()
                .map(|r| r.into_iter().map(String::from).collect())
                .collect(),
            delimiter: b',',
            trailing_newline: true,
        })
    }

    fn handler_no_headers(rows: Vec<Vec<&str>>) -> CsvHandler {
        CsvHandler::new(CsvData {
            headers: None,
            rows: rows
                .into_iter()
                .map(|r| r.into_iter().map(String::from).collect())
                .collect(),
            delimiter: b',',
            trailing_newline: true,
        })
    }

    fn cell_loc(row: usize, col: usize) -> TabularLocation {
        TabularLocation::builder()
            .with_row_index(row)
            .with_column_index(col)
            .build()
            .unwrap()
    }

    fn cell_range(row: usize, col: usize, start: usize, end: usize) -> TabularLocation {
        TabularLocation {
            start_offset: Some(start),
            end_offset: Some(end),
            ..cell_loc(row, col)
        }
    }

    #[tokio::test]
    async fn locations_yield_header_then_rows() {
        let h = handler_with_headers(vec!["name", "age"], vec![vec!["Alice", "30"]]);
        let items: Vec<_> = h.locations().collect().await;
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].location.row_index, 0);
        assert_eq!(items[0].location.column_index, 0);
        assert_eq!(items[0].location.column_name.as_deref(), Some("name"));
        assert_eq!(items[2].location.row_index, 1); // first data row
        assert_eq!(items[2].location.column_index, 0);
    }

    #[tokio::test]
    async fn locations_no_headers_start_at_row_zero() {
        let h = handler_no_headers(vec![vec!["a", "b"], vec!["c", "d"]]);
        let items: Vec<_> = h.locations().collect().await;
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].location.row_index, 0);
        assert_eq!(items[2].location.row_index, 1);
    }

    #[tokio::test]
    async fn read_returns_cell_value() {
        let h = handler_with_headers(vec!["name"], vec![vec!["Alice"]]);
        // header
        assert_eq!(h.read(&cell_loc(0, 0)).await.unwrap().as_str(), "name");
        // first data row
        assert_eq!(h.read(&cell_loc(1, 0)).await.unwrap().as_str(), "Alice");
    }

    #[tokio::test]
    async fn read_out_of_bounds_returns_none() {
        let h = handler_with_headers(vec!["a"], vec![vec!["1"]]);
        assert!(h.read(&cell_loc(99, 0)).await.is_none());
        assert!(h.read(&cell_loc(0, 99)).await.is_none());
    }

    #[tokio::test]
    async fn redact_full_cell() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["ssn"], vec![vec!["123-45-6789"]]);
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            cell_range(1, 0, 0, 11),
            TabularRedaction::new(TextOutput::replace("[REDACTED]")),
        )
        .unwrap();
        h.redact(rs).await?;
        assert_eq!(h.cell(0, 0), Some("[REDACTED]"));
        Ok(())
    }

    #[tokio::test]
    async fn redact_partial_cell() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["bio"], vec![vec!["Alice Smith"]]);
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            cell_range(1, 0, 0, 5),
            TabularRedaction::new(TextOutput::replace("[NAME]")),
        )
        .unwrap();
        h.redact(rs).await?;
        assert_eq!(h.cell(0, 0), Some("[NAME] Smith"));
        Ok(())
    }

    #[tokio::test]
    async fn redact_header() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["secret_field"], vec![vec!["v"]]);
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            cell_range(0, 0, 0, 12),
            TabularRedaction::new(TextOutput::replace("redacted")),
        )
        .unwrap();
        h.redact(rs).await?;
        assert_eq!(h.headers(), Some(["redacted".to_string()].as_slice()));
        Ok(())
    }

    #[tokio::test]
    async fn redact_unknown_row_skipped() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["a"], vec![vec!["one"]]);
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            cell_range(99, 0, 0, 1),
            TabularRedaction::new(TextOutput::replace("X")),
        )
        .unwrap();
        h.redact(rs).await?;
        assert_eq!(h.cell(0, 0), Some("one"));
        Ok(())
    }

    #[test]
    fn encode_with_headers() -> Result<(), Error> {
        let h = handler_with_headers(
            vec!["name", "age"],
            vec![vec!["Alice", "30"], vec!["Bob", "25"]],
        );
        let content = h.encode()?;
        assert_eq!(
            content.as_str().expect("valid utf-8"),
            "name,age\nAlice,30\nBob,25\n"
        );
        Ok(())
    }

    #[test]
    fn encode_with_quoting() -> Result<(), Error> {
        let h = handler_with_headers(vec!["name", "bio"], vec![vec!["Alice", "Has a, comma"]]);
        let content = h.encode()?;
        let text = content.as_str().expect("valid utf-8");
        assert!(text.contains("\"Has a, comma\""));
        Ok(())
    }

    #[test]
    fn encode_without_trailing_newline() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["a"], vec![vec!["1"]]);
        h.data.trailing_newline = false;
        let content = h.encode()?;
        assert_eq!(content.as_str().expect("valid utf-8"), "a\n1");
        Ok(())
    }
}
