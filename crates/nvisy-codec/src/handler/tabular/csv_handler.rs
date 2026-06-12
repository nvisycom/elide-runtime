//! CSV handler: holds parsed CSV content and streams cell coordinates
//! via [`Handler<Tabular>`], with random-access reads / redactions via
//! [`Handler<Tabular>`].
//!
//! Cell coordinates are `(row, col)`. Row 0 is the header row (if
//! present); row 1 is the first data row when headers exist, else row
//! 0 is the first data row. Intra-cell byte offsets on a
//! [`TabularLocation`] address sub-strings within a cell value;
//! omitting them redacts the whole cell.

use std::ops::Range;

use nvisy_core::Error;
use nvisy_core::modality::{Tabular, TabularLocation, TextData};
use nvisy_core::redaction::{Redactions, TabularReplacement};

use super::CsvLoader;
use crate::content::{ContentData, ContentSource};
use crate::handler::tabular::TabularHandle;
use crate::handler::text::redact;
use crate::{Chunk, Format, FormatId, Handler};

const TARGET: &str = "nvisy_codec::handler::tabular::csv";

/// Stable [`FormatId`] for the CSV codec.
pub const FORMAT_ID: FormatId = FormatId::from_static("nvisy.tabular.csv");

/// [`Format`] descriptor registered into [`crate::CodecRegistry`].
pub fn format() -> Format {
    Format::new::<Tabular, _>(FORMAT_ID.clone(), CsvLoader::default())
        .with_extensions(["csv"])
        .with_content_types(["text/csv"])
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

/// Handler for loaded CSV content. Cell coordinates double as the
/// index — no derived offset table to maintain.
#[derive(Debug)]
pub struct CsvHandler {
    source: ContentSource,
    data: CsvData,
    cursor: CsvCursor,
}

/// Streaming cursor over a CSV: walks header row (if present) then
/// data rows, one cell at a time in row-major order.
#[derive(Debug, Default)]
struct CsvCursor {
    row: u32,
    col: u32,
}

#[async_trait::async_trait]
impl Handler<Tabular> for CsvHandler {
    fn format(&self) -> FormatId {
        FORMAT_ID.clone()
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

    async fn next_chunk(&mut self) -> Result<Option<Chunk<Tabular>>, Error> {
        let total_rows = if self.data.headers.is_some() {
            self.data.rows.len() as u32 + 1
        } else {
            self.data.rows.len() as u32
        };

        if self.cursor.row >= total_rows {
            return Ok(None);
        }
        let row_len = self.row_len(self.cursor.row).unwrap_or(0);
        if (self.cursor.col as usize) >= row_len {
            self.cursor.row += 1;
            self.cursor.col = 0;
            return Box::pin(self.next_chunk()).await;
        }

        let row = self.cursor.row;
        let col = self.cursor.col;
        let location = TabularLocation {
            row_index: row,
            column_index: col,
            start_offset: None,
            end_offset: None,
            column_name: self
                .data
                .headers
                .as_ref()
                .and_then(|h| h.get(col as usize).cloned()),
            sheet_name: None,
        };
        let cell = self.cell_at(row, col).expect("bounds checked above");
        let data = TextData::from(cell.to_owned());

        self.cursor.col += 1;
        Ok(Some(Chunk { location, data }))
    }

    fn lift_chunk(
        &self,
        chunk: &Chunk<Tabular>,
        value_range: Range<usize>,
    ) -> Option<TabularLocation> {
        let cell = self.cell_at(chunk.location.row_index, chunk.location.column_index)?;
        if value_range.start > value_range.end || value_range.end > cell.len() {
            return None;
        }
        Some(TabularLocation {
            row_index: chunk.location.row_index,
            column_index: chunk.location.column_index,
            start_offset: Some(value_range.start),
            end_offset: Some(value_range.end),
            column_name: chunk.location.column_name.clone(),
            sheet_name: chunk.location.sheet_name.clone(),
        })
    }

    async fn read(&self, location: &TabularLocation) -> Result<Option<TextData>, Error> {
        Ok(self
            .cell_at(location.row_index, location.column_index)
            .map(|s| TextData::from(s.to_owned())))
    }

    async fn redact(&mut self, mut redactions: Redactions<Tabular>) -> Result<(), Error> {
        // Multiple redactions can target intra-cell byte ranges within
        // the same cell; apply right-to-left over byte offsets so an
        // earlier shrink doesn't invalidate later offsets.
        redactions.sort_descending();
        for (location, replacement) in redactions.into_items() {
            self.redact_one(&location, replacement)?;
        }
        Ok(())
    }
}

impl TabularHandle for CsvHandler {
    fn has_header(&self) -> bool {
        self.data.headers.is_some()
    }
}

impl CsvHandler {
    /// Create a new handler from parsed CSV data.
    pub fn new(data: CsvData) -> Self {
        Self {
            source: ContentSource::new(),
            data,
            cursor: CsvCursor::default(),
        }
    }

    /// Attach a content source for lineage tracking.
    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }

    /// Column headers, if present.
    pub fn headers(&self) -> Option<&[String]> {
        self.data.headers.as_deref()
    }

    /// All data rows (excluding the header row).
    pub fn rows(&self) -> &[Vec<String>] {
        &self.data.rows
    }

    /// Mutable access to all data rows.
    pub fn rows_mut(&mut self) -> &mut Vec<Vec<String>> {
        &mut self.data.rows
    }

    /// A specific cell by `(data_row, col)`. `data_row` is 0-based
    /// against the data rows; the header is *not* data row 0. Use
    /// [`Handler::read`] with [`TabularLocation`] if you need
    /// to address the header.
    pub fn cell(&self, data_row: usize, col: usize) -> Option<&str> {
        self.data
            .rows
            .get(data_row)
            .and_then(|r| r.get(col))
            .map(String::as_str)
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

    /// Rewind the streaming cursor to the start of the document.
    pub fn rewind(&mut self) {
        self.cursor = CsvCursor::default();
    }

    /// Consume the handler and return the inner [`CsvData`].
    pub fn into_data(self) -> CsvData {
        self.data
    }

    fn cell_at(&self, row_index: u32, column_index: u32) -> Option<&str> {
        let col = column_index as usize;
        if self.data.headers.is_some() && row_index == 0 {
            return self.data.headers.as_ref()?.get(col).map(String::as_str);
        }
        let data_row = if self.data.headers.is_some() {
            (row_index - 1) as usize
        } else {
            row_index as usize
        };
        self.data.rows.get(data_row)?.get(col).map(String::as_str)
    }

    fn cell_at_mut(&mut self, row_index: u32, column_index: u32) -> Option<&mut String> {
        let col = column_index as usize;
        if self.data.headers.is_some() && row_index == 0 {
            return self.data.headers.as_mut()?.get_mut(col);
        }
        let data_row = if self.data.headers.is_some() {
            (row_index - 1) as usize
        } else {
            row_index as usize
        };
        self.data.rows.get_mut(data_row)?.get_mut(col)
    }

    fn row_len(&self, row_index: u32) -> Option<usize> {
        if self.data.headers.is_some() && row_index == 0 {
            return self.data.headers.as_ref().map(|h| h.len());
        }
        let data_row = if self.data.headers.is_some() {
            (row_index - 1) as usize
        } else {
            row_index as usize
        };
        self.data.rows.get(data_row).map(|r| r.len())
    }

    fn redact_one(
        &mut self,
        location: &TabularLocation,
        replacement: TabularReplacement,
    ) -> Result<(), Error> {
        let Some(cell) = self.cell_at_mut(location.row_index, location.column_index) else {
            return Ok(());
        };
        let start = location.start_offset.unwrap_or(0);
        let end = location.end_offset.unwrap_or(cell.len());
        let value = replacement.replacement_value().unwrap_or_default();
        redact::replace_range(cell, value, start..end, TARGET)
    }

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
    use nvisy_core::Error;

    use super::*;

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

    fn cell_range(row: u32, col: u32, start: usize, end: usize) -> TabularLocation {
        TabularLocation {
            start_offset: Some(start),
            end_offset: Some(end),
            ..TabularLocation::new(row, col)
        }
    }

    #[tokio::test]
    async fn stream_walks_headers_then_rows() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["name", "age"], vec![vec!["Alice", "30"]]);
        let first = h.next_chunk().await?.unwrap();
        assert_eq!(first.location.row_index, 0);
        assert_eq!(first.location.column_index, 0);
        assert_eq!(first.location.column_name.as_deref(), Some("name"));
        // Drain the rest and count.
        let mut count = 1;
        while h.next_chunk().await?.is_some() {
            count += 1;
        }
        assert_eq!(count, 4); // 2 header + 2 data
        Ok(())
    }

    #[tokio::test]
    async fn stream_no_headers_starts_at_row_zero() -> Result<(), Error> {
        let mut h = handler_no_headers(vec![vec!["a", "b"], vec!["c", "d"]]);
        let first = h.next_chunk().await?.unwrap();
        assert_eq!(first.location.row_index, 0);
        assert_eq!(first.location.column_index, 0);
        assert!(first.location.column_name.is_none());
        let mut count = 1;
        while h.next_chunk().await?.is_some() {
            count += 1;
        }
        assert_eq!(count, 4);
        Ok(())
    }

    #[tokio::test]
    async fn read_returns_cell_value() -> Result<(), Error> {
        let h = handler_with_headers(vec!["name"], vec![vec!["Alice"]]);
        assert_eq!(
            h.read(&TabularLocation::new(0, 0)).await?.unwrap().as_str(),
            "name"
        );
        assert_eq!(
            h.read(&TabularLocation::new(1, 0)).await?.unwrap().as_str(),
            "Alice"
        );
        Ok(())
    }

    #[tokio::test]
    async fn read_out_of_bounds_returns_none() -> Result<(), Error> {
        let h = handler_with_headers(vec!["a"], vec![vec!["1"]]);
        assert!(h.read(&TabularLocation::new(99, 0)).await?.is_none());
        assert!(h.read(&TabularLocation::new(0, 99)).await?.is_none());
        Ok(())
    }

    /// Lifting a recognizer-emitted intra-cell range turns into a
    /// TabularLocation whose row/col match the chunk and whose
    /// start_offset/end_offset point at the substring. Round-trips
    /// through `read` (whole-cell) and a partial-cell redact.
    #[tokio::test]
    async fn lift_chunk_addresses_intra_cell_range() -> Result<(), Error> {
        // Data row 1 col 1 is the email cell.
        let mut h = handler_with_headers(
            vec!["name", "email"],
            vec![vec!["Alice", "alice@example.com"]],
        );
        // Advance the cursor to the email cell.
        let chunk = loop {
            let c = h.next_chunk().await?.expect("chunk");
            if c.data.as_str() == "alice@example.com" {
                break c;
            }
        };
        // Recognizer says `alice` starts at byte 0 within the cell.
        let lifted = h
            .lift_chunk(&chunk, 0.."alice".len())
            .expect("range in bounds");
        assert_eq!(lifted.row_index, 1);
        assert_eq!(lifted.column_index, 1);
        assert_eq!(lifted.column_name.as_deref(), Some("email"));
        assert_eq!(lifted.start_offset, Some(0));
        assert_eq!(lifted.end_offset, Some(5));

        // Out of bounds returns None.
        assert!(h.lift_chunk(&chunk, 0..9999).is_none());
        assert!(h.lift_chunk(&chunk, 99..100).is_none());
        Ok(())
    }

    /// Pipeline: lift a recognizer-emitted intra-cell range, push it
    /// through `redact`, and confirm only the matched substring
    /// changes (not the whole cell).
    #[tokio::test]
    async fn lift_chunk_into_partial_cell_redact() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["email"], vec![vec!["alice@example.com"]]);
        let chunk = loop {
            let c = h.next_chunk().await?.expect("chunk");
            if c.data.as_str() == "alice@example.com" {
                break c;
            }
        };
        let lifted = h
            .lift_chunk(&chunk, 0.."alice".len())
            .expect("range in bounds");
        let mut rs = Redactions::new();
        rs.push(lifted, TabularReplacement::substituted("[USER]"));
        h.redact(rs).await?;
        let encoded = h.encode()?.as_str().unwrap().to_owned();
        assert!(
            encoded.contains("[USER]@example.com"),
            "partial cell redaction lost: {encoded}",
        );
        Ok(())
    }

    #[tokio::test]
    async fn redact_full_cell() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["ssn"], vec![vec!["123-45-6789"]]);
        let mut rs = Redactions::new();
        rs.push(
            cell_range(1, 0, 0, 11),
            TabularReplacement::substituted("[REDACTED]"),
        );
        h.redact(rs).await?;
        assert_eq!(h.cell(0, 0), Some("[REDACTED]"));
        Ok(())
    }

    #[tokio::test]
    async fn redact_partial_cell() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["bio"], vec![vec!["Alice Smith"]]);
        let mut rs = Redactions::new();
        rs.push(
            cell_range(1, 0, 0, 5),
            TabularReplacement::substituted("[NAME]"),
        );
        h.redact(rs).await?;
        assert_eq!(h.cell(0, 0), Some("[NAME] Smith"));
        Ok(())
    }

    #[tokio::test]
    async fn redact_header() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["secret_field"], vec![vec!["v"]]);
        let mut rs = Redactions::new();
        rs.push(
            cell_range(0, 0, 0, 12),
            TabularReplacement::substituted("redacted"),
        );
        h.redact(rs).await?;
        assert_eq!(h.headers(), Some(["redacted".to_string()].as_slice()));
        Ok(())
    }

    #[tokio::test]
    async fn redact_two_ranges_same_cell() -> Result<(), Error> {
        // Two intra-cell ranges within "Alice Smith"; expected right-to-left.
        let mut h = handler_with_headers(vec!["bio"], vec![vec!["Alice Smith"]]);
        let mut rs = Redactions::new();
        rs.push(
            cell_range(1, 0, 0, 5),
            TabularReplacement::substituted("[A]"),
        );
        rs.push(
            cell_range(1, 0, 6, 11),
            TabularReplacement::substituted("[B]"),
        );
        h.redact(rs).await?;
        assert_eq!(h.cell(0, 0), Some("[A] [B]"));
        Ok(())
    }

    #[tokio::test]
    async fn redact_unknown_row_skipped() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["a"], vec![vec!["one"]]);
        let mut rs = Redactions::new();
        rs.push(
            cell_range(99, 0, 0, 1),
            TabularReplacement::substituted("X"),
        );
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
