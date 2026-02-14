//! CSV handler — holds parsed CSV content and provides span-based
//! access via [`Handler`].
//!
//! The handler stores the parsed rows (and optional headers) together
//! with the detected delimiter so the file can be reconstructed after
//! edits.
//!
//! # Span model
//!
//! [`Handler::view_spans`] yields one [`Span`] per cell.  If headers
//! are present, header cells are emitted first (with
//! [`CsvSpan::header`] set to `true`), followed by data cells in
//! row-major order.
//!
//! [`Handler::edit_spans`] replaces cell content at the given
//! (row, col) position.  Header cells can also be edited.

use futures::StreamExt;

use nvisy_core::error::Error;
use nvisy_ontology::entity::DocumentType;

use crate::document::edit_stream::SpanEditStream;
use crate::document::view_stream::SpanStream;
use crate::handler::{Handler, Span};

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
#[derive(Debug)]
pub struct CsvHandler {
    pub(crate) data: CsvData,
}

#[async_trait::async_trait]
impl Handler for CsvHandler {
    fn document_type(&self) -> DocumentType {
        DocumentType::Csv
    }

    type SpanId = CsvSpan;
    type SpanData = String;

    async fn view_spans(&self) -> SpanStream<'_, CsvSpan, String> {
        SpanStream::new(futures::stream::iter(CsvSpanIter::new(&self.data)))
    }

    async fn edit_spans(
        &mut self,
        edits: SpanEditStream<'_, CsvSpan, String>,
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
                *cell = edit.data;
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
                *cell = edit.data;
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

    /// A specific cell by (row, col).
    pub fn cell(&self, row: usize, col: usize) -> Option<&str> {
        self.data
            .rows
            .get(row)
            .and_then(|r| r.get(col))
            .map(|s| s.as_str())
    }

    /// Number of data rows (excluding the header).
    pub fn row_count(&self) -> usize {
        self.data.rows.len()
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
    type Item = Span<CsvSpan, String>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match &self.phase {
                CsvIterPhase::Headers => {
                    let headers = self.headers?;
                    if let Some(value) = headers.get(self.col) {
                        let col = self.col;
                        self.col += 1;
                        return Some(Span {
                            id: CsvSpan::header_cell(col, value.clone()),
                            data: value.clone(),
                        });
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
                        return Some(Span {
                            id: CsvSpan::cell(row_idx, col, key),
                            data: value.clone(),
                        });
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
    use crate::handler::SpanEdit;
    use futures::StreamExt;

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
        let spans: Vec<_> = h.view_spans().await.collect().await;

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
        let spans: Vec<_> = h.view_spans().await.collect().await;

        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0].id, CsvSpan::cell(0, 0, "0"));
        assert_eq!(spans[0].id.key, "0");
        assert_eq!(spans[0].data, "x");
    }

    #[tokio::test]
    async fn view_spans_empty() {
        let h = handler_no_headers(vec![]);
        let spans: Vec<_> = h.view_spans().await.collect().await;
        assert!(spans.is_empty());
    }

    #[tokio::test]
    async fn edit_spans_data_cell() {
        let mut h = handler_with_headers(
            vec!["ssn"],
            vec![vec!["123-45-6789"]],
        );
        h.edit_spans(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit {
                id: CsvSpan::cell(0, 0, "ssn"),
                data: "[REDACTED]".into(),
            },
        ])))
        .await
        .unwrap();
        assert_eq!(h.cell(0, 0), Some("[REDACTED]"));
    }

    #[tokio::test]
    async fn edit_spans_header_cell() {
        let mut h = handler_with_headers(
            vec!["secret_field"],
            vec![vec!["value"]],
        );
        h.edit_spans(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit {
                id: CsvSpan::header_cell(0, "secret_field"),
                data: "redacted".into(),
            },
        ])))
        .await
        .unwrap();
        assert_eq!(h.headers(), Some(["redacted".to_string()].as_slice()));
    }

    #[tokio::test]
    async fn edit_spans_row_out_of_bounds() {
        let mut h = handler_no_headers(vec![vec!["a"]]);
        let err = h
            .edit_spans(SpanEditStream::new(futures::stream::iter(vec![
                SpanEdit {
                    id: CsvSpan::cell(5, 0, "0"),
                    data: "x".into(),
                },
            ])))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
    }

    #[tokio::test]
    async fn edit_spans_col_out_of_bounds() {
        let mut h = handler_no_headers(vec![vec!["a"]]);
        let err = h
            .edit_spans(SpanEditStream::new(futures::stream::iter(vec![
                SpanEdit {
                    id: CsvSpan::cell(0, 5, "5"),
                    data: "x".into(),
                },
            ])))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
    }
}
