//! CSV handler: holds parsed CSV content and provides span-based
//! access via [`Handler`] + [`TextHandler`].
//!
//! The handler stores the parsed rows (and optional headers) together
//! with the detected delimiter so the file can be reconstructed after
//! edits.
//!
//! # Span model
//!
//! [`TextHandler::text_spans`] yields one [`Span`] per cell. If headers
//! are present, header cells are emitted first, followed by data cells
//! in row-major order. Each span is addressed by a [`TextLocation`]
//! with byte offsets computed from the **serialized** CSV form,
//! correctly accounting for quoted/escaped fields.
//!
//! # Offset semantics
//!
//! Offsets are into the serialized CSV string (after CRLF→LF
//! normalization). Quoted fields include the quote characters in their
//! offset range. The `value` field on [`TextLocation`] carries the
//! unescaped cell content.

use futures::StreamExt;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::{DocumentType, SpreadsheetFormat};
use nvisy_ontology::entity::TextLocation;

use crate::document::{Span, SpanStream};
use crate::handler::text::TextData;
use crate::handler::{Handler, TextHandler};

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

/// A located CSV cell with structural identity and byte offsets.
struct CellLocation {
    is_header: bool,
    row: usize,
    col: usize,
    value: String,
    start: usize,
    end: usize,
    line_number: u32,
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
impl TextHandler for CsvHandler {
    async fn text_spans(&self) -> SpanStream<'_, TextLocation, TextData> {
        let cells = self.locate_cells();
        let source = self.source;
        let spans: Vec<_> = cells
            .into_iter()
            .map(|c| {
                Span::new(
                    TextLocation {
                        start_offset: c.start,
                        end_offset: c.end,
                        line_number: Some(c.line_number),
                        ..Default::default()
                    },
                    TextData::from(c.value),
                )
                .with_source(source)
            })
            .collect();
        SpanStream::new(futures::stream::iter(spans))
    }

    async fn edit_text(
        &mut self,
        edits: SpanStream<'_, TextLocation, TextData>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        let cells = self.locate_cells();
        for edit in edits {
            let cell = cells
                .iter()
                .find(|c| c.start == edit.id.start_offset && c.end == edit.id.end_offset)
                .ok_or_else(|| {
                    Error::validation(
                        format!(
                            "no cell at byte offset {}..{}",
                            edit.id.start_offset, edit.id.end_offset
                        ),
                        "csv-handler",
                    )
                })?;

            if cell.is_header {
                let headers = self
                    .data
                    .headers
                    .as_mut()
                    .ok_or_else(|| Error::validation("no headers to edit", "csv-handler"))?;
                headers[cell.col] = edit.data.into_inner();
            } else {
                let row = self.data.rows.get_mut(cell.row).ok_or_else(|| {
                    Error::validation(format!("row {} out of bounds", cell.row), "csv-handler")
                })?;
                let target = row.get_mut(cell.col).ok_or_else(|| {
                    Error::validation(
                        format!("col {} out of bounds in row {}", cell.col, cell.row),
                        "csv-handler",
                    )
                })?;
                *target = edit.data.into_inner();
            }
        }
        Ok(())
    }

    async fn value_at(&self, location: &TextLocation) -> Option<String> {
        let cells = self.locate_cells();
        cells
            .iter()
            .find(|c| c.start == location.start_offset && c.end == location.end_offset)
            .map(|c| c.value.clone())
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

    /// A specific cell by (row, col).
    pub fn cell(&self, row: usize, col: usize) -> Option<&str> {
        self.data
            .rows
            .get(row)
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

    /// Serialize to bytes (with CRLF→LF normalization and trailing
    /// newline handling).
    fn serialize_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut wtr = csv::WriterBuilder::new()
            .delimiter(self.data.delimiter)
            .has_headers(false)
            .from_writer(Vec::new());

        if let Some(headers) = &self.data.headers {
            wtr.write_record(headers)
                .map_err(|e| Error::validation(format!("CSV encode error: {e}"), "csv-handler"))?;
        }
        for row in &self.data.rows {
            wtr.write_record(row)
                .map_err(|e| Error::validation(format!("CSV encode error: {e}"), "csv-handler"))?;
        }
        let mut bytes = wtr
            .into_inner()
            .map_err(|e| Error::validation(format!("CSV encode error: {e}"), "csv-handler"))?;

        bytes.retain(|&b| b != b'\r');

        if !self.data.trailing_newline && bytes.last() == Some(&b'\n') {
            bytes.pop();
        }

        Ok(bytes)
    }

    /// Locate all cells by serializing and finding field boundaries.
    ///
    /// Serializes the CSV once, splits by newlines, and uses a
    /// field-position parser on each line that correctly handles
    /// quoted/escaped fields.
    fn locate_cells(&self) -> Vec<CellLocation> {
        let bytes = match self.serialize_bytes() {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        };
        let text = String::from_utf8_lossy(&bytes);

        let mut cells = Vec::new();
        let has_headers = self.data.headers.is_some();
        let mut absolute_offset = 0usize;

        for (row_idx, line) in text.split('\n').enumerate() {
            if line.is_empty() {
                continue;
            }

            let is_header = has_headers && row_idx == 0;
            let data_row = if is_header {
                0
            } else if has_headers {
                row_idx - 1
            } else {
                row_idx
            };
            let line_num = (row_idx + 1) as u32;

            let row_values = if is_header {
                self.data.headers.as_deref().unwrap_or(&[])
            } else {
                self.data
                    .rows
                    .get(data_row)
                    .map(|r| r.as_slice())
                    .unwrap_or(&[])
            };

            for (col, value) in row_values.iter().enumerate() {
                if let Some((rel_start, rel_end)) =
                    find_field_in_line(line, self.data.delimiter, col)
                {
                    cells.push(CellLocation {
                        is_header,
                        row: data_row,
                        col,
                        value: value.clone(),
                        start: absolute_offset + rel_start,
                        end: absolute_offset + rel_end,
                        line_number: line_num,
                    });
                }
            }

            absolute_offset += line.len() + 1; // +1 for the newline
        }

        cells
    }
}

/// Find a field's byte range within a CSV line, accounting for quoting.
fn find_field_in_line(line: &str, delimiter: u8, target_col: usize) -> Option<(usize, usize)> {
    let mut col = 0usize;
    let mut pos = 0usize;

    while pos < line.len() && col <= target_col {
        if col == target_col {
            // Found the target column.
            if line[pos..].starts_with('"') {
                // Quoted field — find closing quote.
                let content_start = pos;
                pos += 1; // skip opening quote
                loop {
                    if pos >= line.len() {
                        break;
                    }
                    if line.as_bytes()[pos] == b'"' {
                        pos += 1;
                        if pos < line.len() && line.as_bytes()[pos] == b'"' {
                            pos += 1; // escaped quote
                        } else {
                            break; // closing quote
                        }
                    } else {
                        pos += 1;
                    }
                }
                return Some((content_start, pos));
            } else {
                // Unquoted field — find delimiter or end of line.
                let start = pos;
                while pos < line.len()
                    && line.as_bytes()[pos] != delimiter
                    && line.as_bytes()[pos] != b'\n'
                {
                    pos += 1;
                }
                return Some((start, pos));
            }
        }

        // Skip to next field.
        if line[pos..].starts_with('"') {
            pos += 1;
            loop {
                if pos >= line.len() {
                    break;
                }
                if line.as_bytes()[pos] == b'"' {
                    pos += 1;
                    if pos < line.len() && line.as_bytes()[pos] == b'"' {
                        pos += 1;
                    } else {
                        break;
                    }
                } else {
                    pos += 1;
                }
            }
        } else {
            while pos < line.len()
                && line.as_bytes()[pos] != delimiter
                && line.as_bytes()[pos] != b'\n'
            {
                pos += 1;
            }
        }

        if pos < line.len() && line.as_bytes()[pos] == delimiter {
            pos += 1; // skip delimiter
        }
        col += 1;
    }

    None
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use nvisy_core::Error;

    use super::*;
    use crate::document::Span;
    use crate::handler::TextHandler;

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

    #[tokio::test]
    async fn view_spans_with_headers() {
        let h = handler_with_headers(
            vec!["name", "age"],
            vec![vec!["Alice", "30"], vec!["Bob", "25"]],
        );
        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans.len(), 6);
        assert_eq!(spans[0].data, "name");
        assert_eq!(spans[1].data, "age");
        assert_eq!(spans[2].data, "Alice");
        assert_eq!(spans[3].data, "30");
        assert_eq!(spans[4].data, "Bob");
        assert_eq!(spans[5].data, "25");
    }

    #[tokio::test]
    async fn view_spans_no_headers() {
        let h = handler_no_headers(vec![vec!["x", "y"], vec!["1", "2"]]);
        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0].data, "x");
    }

    #[tokio::test]
    async fn edit_cell() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["ssn"], vec![vec!["123-45-6789"]]);
        let spans: Vec<_> = h.text_spans().await.collect().await;
        let data_loc = spans[1].id.clone();
        h.edit_text(SpanStream::new(futures::stream::iter(vec![Span::new(
            data_loc,
            "[REDACTED]".into(),
        )])))
        .await?;
        assert_eq!(h.cell(0, 0), Some("[REDACTED]"));
        Ok(())
    }

    #[tokio::test]
    async fn edit_header() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["secret_field"], vec![vec!["value"]]);
        let spans: Vec<_> = h.text_spans().await.collect().await;
        let header_loc = spans[0].id.clone();
        h.edit_text(SpanStream::new(futures::stream::iter(vec![Span::new(
            header_loc,
            "redacted".into(),
        )])))
        .await?;
        assert_eq!(h.headers(), Some(["redacted".to_string()].as_slice()));
        Ok(())
    }

    #[tokio::test]
    async fn value_at_returns_cell() {
        let h = handler_with_headers(vec!["name"], vec![vec!["Alice"]]);
        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(h.value_at(&spans[1].id).await, Some("Alice".to_string()));
    }

    #[tokio::test]
    async fn quoted_field_offsets_correct() {
        let h = handler_with_headers(vec!["bio"], vec![vec!["has, comma"]]);
        let spans: Vec<_> = h.text_spans().await.collect().await;
        let bio_span = spans.iter().find(|s| s.data.as_str() == "has, comma");
        assert!(bio_span.is_some(), "should find quoted field");
        let loc = &bio_span.unwrap().id;
        // Offsets should include the quotes in the serialized form.
        assert!(loc.end_offset > loc.start_offset);
        assert_eq!(h.value_at(loc).await, Some("has, comma".to_string()));
    }

    #[tokio::test]
    async fn empty_data_with_headers() {
        let h = handler_with_headers(vec!["a", "b"], vec![]);
        let spans: Vec<_> = h.text_spans().await.collect().await;
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].data, "a");
        assert_eq!(spans[1].data, "b");
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
