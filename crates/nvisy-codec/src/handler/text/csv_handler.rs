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
//! with byte offsets computed from the serialized CSV.

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

impl Handler for CsvHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Spreadsheet(SpreadsheetFormat::Csv)
    }

    fn source(&self) -> ContentSource {
        self.source
    }

    #[tracing::instrument(name = "csv.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<ContentData, Error> {
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

        // Normalize CRLF → LF
        bytes.retain(|&b| b != b'\r');

        // Handle trailing newline
        if !self.data.trailing_newline && bytes.last() == Some(&b'\n') {
            bytes.pop();
        }

        tracing::Span::current().record("output_bytes", bytes.len());
        let source = ContentSource::new().with_parent(&self.source);
        Ok(ContentData::new(source, bytes.into()))
    }
}

#[async_trait::async_trait]
impl TextHandler for CsvHandler {
    async fn text_spans(&self) -> SpanStream<'_, TextLocation, TextData> {
        let cells = self.collect_cells();
        SpanStream::new(futures::stream::iter(cells))
    }

    async fn edit_text(
        &mut self,
        edits: SpanStream<'_, TextLocation, TextData>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        let cell_map = self.cell_locations();
        for edit in edits {
            let (is_header, row, col) = cell_map
                .iter()
                .find(|(_, _, _, loc)| {
                    loc.start_offset == edit.id.start_offset
                        && loc.end_offset == edit.id.end_offset
                })
                .map(|&(h, r, c, _)| (h, r, c))
                .ok_or_else(|| {
                    Error::validation(
                        format!(
                            "no cell at byte offset {}..{}",
                            edit.id.start_offset, edit.id.end_offset
                        ),
                        "csv-handler",
                    )
                })?;

            if is_header {
                let headers = self
                    .data
                    .headers
                    .as_mut()
                    .ok_or_else(|| Error::validation("no headers to edit", "csv-handler"))?;
                headers[col] = edit.data.into_inner();
            } else {
                let row_data = self.data.rows.get_mut(row).ok_or_else(|| {
                    Error::validation(format!("row {row} out of bounds"), "csv-handler")
                })?;
                let cell = row_data.get_mut(col).ok_or_else(|| {
                    Error::validation(
                        format!("column {col} out of bounds in row {row}"),
                        "csv-handler",
                    )
                })?;
                *cell = edit.data.into_inner();
            }
        }
        Ok(())
    }

    async fn value_at(&self, location: &TextLocation) -> Option<String> {
        let cells = self.cell_locations();
        cells
            .iter()
            .find(|(_, _, _, loc)| {
                loc.start_offset == location.start_offset
                    && loc.end_offset == location.end_offset
            })
            .and_then(|&(is_header, row, col, _)| {
                if is_header {
                    self.data.headers.as_ref()?.get(col).cloned()
                } else {
                    self.data.rows.get(row)?.get(col).cloned()
                }
            })
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

    /// Collect all cells as spans with computed byte-offset locations.
    fn collect_cells(&self) -> Vec<Span<TextLocation, TextData>> {
        let mut spans = Vec::new();
        let mut offset = 0usize;

        if let Some(headers) = &self.data.headers {
            for value in headers {
                let start = offset;
                let end = start + value.len();
                spans.push(
                    Span::new(
                        TextLocation {
                            start_offset: start,
                            end_offset: end,
                            line_number: Some(1),
                            ..Default::default()
                        },
                        TextData::from(value.clone()),
                    )
                    .with_source(self.source),
                );
                // +1 for delimiter or newline separator.
                offset = end + 1;
            }
        }

        for (row_idx, row) in self.data.rows.iter().enumerate() {
            let line_num = if self.data.headers.is_some() {
                row_idx + 2
            } else {
                row_idx + 1
            };
            for value in row {
                let start = offset;
                let end = start + value.len();
                spans.push(
                    Span::new(
                        TextLocation {
                            start_offset: start,
                            end_offset: end,
                            line_number: Some(line_num as u32),
                            ..Default::default()
                        },
                        TextData::from(value.clone()),
                    )
                    .with_source(self.source),
                );
                offset = end + 1;
            }
        }

        spans
    }

    /// Compute `(is_header, row, col, TextLocation)` for each cell.
    fn cell_locations(&self) -> Vec<(bool, usize, usize, TextLocation)> {
        let mut locs = Vec::new();
        let mut offset = 0usize;

        if let Some(headers) = &self.data.headers {
            for (col, value) in headers.iter().enumerate() {
                let start = offset;
                let end = start + value.len();
                locs.push((
                    true,
                    0,
                    col,
                    TextLocation {
                        start_offset: start,
                        end_offset: end,
                        ..Default::default()
                    },
                ));
                offset = end + 1;
            }
        }

        for (row_idx, row) in self.data.rows.iter().enumerate() {
            for (col, value) in row.iter().enumerate() {
                let start = offset;
                let end = start + value.len();
                locs.push((
                    false,
                    row_idx,
                    col,
                    TextLocation {
                        start_offset: start,
                        end_offset: end,
                        ..Default::default()
                    },
                ));
                offset = end + 1;
            }
        }

        locs
    }
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

        // 2 header cells + 4 data cells
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
    async fn edit_spans_data_cell() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["ssn"], vec![vec!["123-45-6789"]]);
        let spans: Vec<_> = h.text_spans().await.collect().await;
        // The data cell is the second span (after the header).
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
    async fn edit_spans_header_cell() -> Result<(), Error> {
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
    fn encode_without_trailing_newline() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["a"], vec![vec!["1"]]);
        h.data.trailing_newline = false;
        let content = h.encode()?;
        assert_eq!(content.as_str().expect("valid utf-8"), "a\n1");
        Ok(())
    }
}
