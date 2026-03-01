//! CSV handler: holds parsed CSV content and provides span-based
//! access via [`Handler`] + [`TextHandler`].
//!
//! The handler stores the parsed rows (and optional headers) together
//! with the detected delimiter so the file can be reconstructed after
//! edits.
//!
//! # Span model
//!
//! [`TextHandler::text_spans`] yields one [`Span`] per cell.  If headers
//! are present, header cells are emitted first (with
//! [`CsvSpan::header`] set to `true`), followed by data cells in
//! row-major order.
//!
//! [`TextHandler::edit_text`] replaces cell content at the given
//! (row, col) position.  Header cells can also be edited.

use futures::StreamExt;

use nvisy_core::Error;
use nvisy_core::fs::DocumentType;

use crate::handler::{Handler, Span, SpanEditStream, SpanStream, TextHandler};
use crate::handler::text::TextData;

/// Cell address within a CSV document.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CsvSpan {
    /// 0-based row index (within data rows, not counting the header).
    pub row: usize,
    /// 0-based column index.
    pub col: usize,
    /// `true` when this span addresses a header cell rather than a
    /// data cell.
    pub header: bool,
    /// Column name (from the header row) or column index as a string
    /// when no headers are present.
    pub key: String,
}

impl CsvSpan {
    /// Address a data cell with a column key.
    pub fn cell(row: usize, col: usize, key: impl Into<String>) -> Self {
        Self {
            row,
            col,
            header: false,
            key: key.into(),
        }
    }

    /// Address a header cell.
    pub fn header_cell(col: usize, key: impl Into<String>) -> Self {
        Self {
            row: 0,
            col,
            header: true,
            key: key.into(),
        }
    }
}

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
#[derive(Debug, Clone)]
pub struct CsvHandler {
    pub(crate) data: CsvData,
}

impl Handler for CsvHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Csv
    }

    #[tracing::instrument(name = "csv.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<bytes::Bytes, Error> {
        let mut wtr = csv::WriterBuilder::new()
            .delimiter(self.data.delimiter)
            .has_headers(false)
            .from_writer(Vec::new());

        if let Some(headers) = &self.data.headers {
            wtr.write_record(headers).map_err(|e| {
                Error::validation(format!("CSV encode error: {e}"), "csv-handler")
            })?;
        }
        for row in &self.data.rows {
            wtr.write_record(row).map_err(|e| {
                Error::validation(format!("CSV encode error: {e}"), "csv-handler")
            })?;
        }
        let mut bytes = wtr.into_inner().map_err(|e| {
            Error::validation(format!("CSV encode error: {e}"), "csv-handler")
        })?;

        // Normalize CRLF → LF
        bytes.retain(|&b| b != b'\r');

        // Handle trailing newline
        if !self.data.trailing_newline && bytes.last() == Some(&b'\n') {
            bytes.pop();
        }

        tracing::Span::current().record("output_bytes", bytes.len());
        Ok(bytes.into())
    }
}

#[async_trait::async_trait]
impl TextHandler for CsvHandler {
    type TextId = CsvSpan;

    async fn text_spans(&self) -> SpanStream<'_, CsvSpan, TextData> {
        SpanStream::new(futures::stream::iter(CsvSpanIter::new(&self.data)))
    }

    async fn edit_text(
        &mut self,
        edits: SpanEditStream<'_, CsvSpan, TextData>,
    ) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        for edit in edits {
            if edit.id.header {
                let headers = self.data.headers.as_mut().ok_or_else(|| {
                    Error::validation("no headers to edit", "csv-handler")
                })?;
                let cell = headers.get_mut(edit.id.col).ok_or_else(|| {
                    Error::validation(
                        format!("header column {} out of bounds", edit.id.col),
                        "csv-handler",
                    )
                })?;
                *cell = edit.data.into_inner();
            } else {
                let row = self.data.rows.get_mut(edit.id.row).ok_or_else(|| {
                    Error::validation(
                        format!("row {} out of bounds", edit.id.row),
                        "csv-handler",
                    )
                })?;
                let cell = row.get_mut(edit.id.col).ok_or_else(|| {
                    Error::validation(
                        format!(
                            "column {} out of bounds in row {}",
                            edit.id.col, edit.id.row,
                        ),
                        "csv-handler",
                    )
                })?;
                *cell = edit.data.into_inner();
            }
        }
        Ok(())
    }
}

impl CsvHandler {
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
}

/// Iterator over cells of a CSV document.
///
/// Yields header cells first (if present), then data cells in
/// row-major order.
struct CsvSpanIter<'a> {
    headers: Option<&'a [String]>,
    rows: &'a [Vec<String>],
    /// Current position: `None` = in headers, `Some(row)` = in data.
    phase: CsvIterPhase,
    col: usize,
}

enum CsvIterPhase {
    Headers,
    Data(usize),
}

impl<'a> CsvSpanIter<'a> {
    fn new(data: &'a CsvData) -> Self {
        let phase = if data.headers.is_some() {
            CsvIterPhase::Headers
        } else {
            CsvIterPhase::Data(0)
        };
        Self {
            headers: data.headers.as_deref(),
            rows: &data.rows,
            phase,
            col: 0,
        }
    }
}

impl<'a> Iterator for CsvSpanIter<'a> {
    type Item = Span<CsvSpan, TextData>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match &self.phase {
                CsvIterPhase::Headers => {
                    let headers = self.headers?;
                    if let Some(value) = headers.get(self.col) {
                        let col = self.col;
                        self.col += 1;
                        return Some(Span::new(
                            CsvSpan::header_cell(col, value.clone()),
                            TextData::from(value.clone()),
                        ));
                    }
                    self.phase = CsvIterPhase::Data(0);
                    self.col = 0;
                }
                CsvIterPhase::Data(row) => {
                    let row_idx = *row;
                    let row_data = self.rows.get(row_idx)?;
                    if let Some(value) = row_data.get(self.col) {
                        let col = self.col;
                        self.col += 1;
                        let key = self
                            .headers
                            .and_then(|h| h.get(col))
                            .cloned()
                            .unwrap_or_else(|| col.to_string());
                        return Some(Span::new(
                            CsvSpan::cell(row_idx, col, key),
                            TextData::from(value.clone()),
                        ));
                    }
                    self.phase = CsvIterPhase::Data(row_idx + 1);
                    self.col = 0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::{SpanEdit, TextHandler};
    use futures::StreamExt;
    use nvisy_core::Error;

    fn handler_with_headers(
        headers: Vec<&str>,
        rows: Vec<Vec<&str>>,
    ) -> CsvHandler {
        CsvHandler {
            data: CsvData {
                headers: Some(headers.into_iter().map(String::from).collect()),
                rows: rows
                    .into_iter()
                    .map(|r| r.into_iter().map(String::from).collect())
                    .collect(),
                delimiter: b',',
                trailing_newline: true,
            },
        }
    }

    fn handler_no_headers(rows: Vec<Vec<&str>>) -> CsvHandler {
        CsvHandler {
            data: CsvData {
                headers: None,
                rows: rows
                    .into_iter()
                    .map(|r| r.into_iter().map(String::from).collect())
                    .collect(),
                delimiter: b',',
                trailing_newline: true,
            },
        }
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

        // Headers
        assert_eq!(spans[0].id, CsvSpan::header_cell(0, "name"));
        assert_eq!(spans[0].data, "name");
        assert_eq!(spans[1].id, CsvSpan::header_cell(1, "age"));
        assert_eq!(spans[1].data, "age");

        // Row 0
        assert_eq!(spans[2].id, CsvSpan::cell(0, 0, "name"));
        assert_eq!(spans[2].id.key, "name");
        assert_eq!(spans[2].data, "Alice");
        assert_eq!(spans[3].id, CsvSpan::cell(0, 1, "age"));
        assert_eq!(spans[3].id.key, "age");
        assert_eq!(spans[3].data, "30");

        // Row 1
        assert_eq!(spans[4].id, CsvSpan::cell(1, 0, "name"));
        assert_eq!(spans[4].data, "Bob");
        assert_eq!(spans[5].id, CsvSpan::cell(1, 1, "age"));
        assert_eq!(spans[5].data, "25");
    }

    #[tokio::test]
    async fn view_spans_no_headers() {
        let h = handler_no_headers(vec![vec!["x", "y"], vec!["1", "2"]]);
        let spans: Vec<_> = h.text_spans().await.collect().await;

        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0].id, CsvSpan::cell(0, 0, "0"));
        assert_eq!(spans[0].id.key, "0");
        assert_eq!(spans[0].data, "x");
    }

    #[tokio::test]
    async fn edit_spans_data_cell() -> Result<(), Error> {
        let mut h = handler_with_headers(
            vec!["ssn"],
            vec![vec!["123-45-6789"]],
        );
        h.edit_text(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit::new(CsvSpan::cell(0, 0, "ssn"), "[REDACTED]".into()),
        ])))
        .await?;
        assert_eq!(h.cell(0, 0), Some("[REDACTED]"));
        Ok(())
    }

    #[tokio::test]
    async fn edit_spans_header_cell() -> Result<(), Error> {
        let mut h = handler_with_headers(
            vec!["secret_field"],
            vec![vec!["value"]],
        );
        h.edit_text(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit::new(CsvSpan::header_cell(0, "secret_field"), "redacted".into()),
        ])))
        .await?;
        assert_eq!(h.headers(), Some(["redacted".to_string()].as_slice()));
        Ok(())
    }

    #[tokio::test]
    async fn edit_spans_row_out_of_bounds() {
        let mut h = handler_no_headers(vec![vec!["a"]]);
        let err = h
            .edit_text(SpanEditStream::new(futures::stream::iter(vec![
                SpanEdit::new(CsvSpan::cell(5, 0, "0"), "x".into()),
            ])))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
    }

    #[tokio::test]
    async fn edit_spans_col_out_of_bounds() {
        let mut h = handler_no_headers(vec![vec!["a"]]);
        let err = h
            .edit_text(SpanEditStream::new(futures::stream::iter(vec![
                SpanEdit::new(CsvSpan::cell(0, 5, "5"), "x".into()),
            ])))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
    }

    #[test]
    fn encode_with_headers() -> Result<(), Error> {
        let h = handler_with_headers(
            vec!["name", "age"],
            vec![vec!["Alice", "30"], vec!["Bob", "25"]],
        );
        let bytes = h.encode()?;
        assert_eq!(
            std::str::from_utf8(&bytes).expect("valid utf-8"),
            "name,age\nAlice,30\nBob,25\n"
        );
        Ok(())
    }

    #[test]
    fn encode_with_quoting() -> Result<(), Error> {
        let h = handler_with_headers(
            vec!["name", "bio"],
            vec![vec!["Alice", "Has a, comma"]],
        );
        let bytes = h.encode()?;
        let text = std::str::from_utf8(&bytes).expect("valid utf-8");
        assert!(text.contains("\"Has a, comma\""));
        Ok(())
    }

    #[test]
    fn encode_without_trailing_newline() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["a"], vec![vec!["1"]]);
        h.data.trailing_newline = false;
        let bytes = h.encode()?;
        assert_eq!(std::str::from_utf8(&bytes).expect("valid utf-8"), "a\n1");
        Ok(())
    }

    #[test]
    fn encode_tab_delimiter() -> Result<(), Error> {
        let mut h = handler_with_headers(
            vec!["a", "b"],
            vec![vec!["1", "2"]],
        );
        h.data.delimiter = b'\t';
        let bytes = h.encode()?;
        assert_eq!(
            std::str::from_utf8(&bytes).expect("valid utf-8"),
            "a\tb\n1\t2\n"
        );
        Ok(())
    }
}
