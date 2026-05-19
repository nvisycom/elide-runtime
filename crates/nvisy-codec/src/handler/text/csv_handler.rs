//! CSV handler: holds parsed CSV content and provides location-based
//! access via [`Handler`] + [`TextHandler`].
//!
//! [`TextHandler::locations`] yields one location per cell, ordered
//! header-then-row-major. Each location's byte offsets address the
//! field in the **serialized** CSV form (quoted/escaped if necessary).
//! [`TextHandler::read`] returns the unescaped cell value.

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::{DocumentType, SpreadsheetFormat};
use nvisy_ontology::entity::TextLocation;

use crate::document::{Located, LocationStream};
use crate::handler::text::TextData;
use crate::handler::{Handler, TextHandler};
use crate::transform::{Redactions, TextRedaction, apply_text_redactions};

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
    fn locations(&self) -> LocationStream<'_, TextLocation> {
        let source = self.source;
        let cells = self.locate_cells();
        let items: Vec<_> = cells
            .into_iter()
            .map(|c| {
                Located::new(
                    source,
                    TextLocation {
                        start_offset: c.start,
                        end_offset: c.end,
                        line_number: Some(c.line_number),
                        ..Default::default()
                    },
                )
            })
            .collect();
        LocationStream::new(futures::stream::iter(items))
    }

    async fn read(&self, location: &TextLocation) -> Option<TextData> {
        self.locate_cells()
            .into_iter()
            .find(|c| c.start == location.start_offset && c.end == location.end_offset)
            .map(|c| TextData::from(c.value))
    }

    async fn redact(
        &mut self,
        redactions: Redactions<TextLocation, TextRedaction>,
    ) -> Result<(), Error> {
        if redactions.is_empty() {
            return Ok(());
        }
        let cells = self.locate_cells();

        let mut updates: Vec<(bool, usize, usize, String)> = Vec::new();
        for (loc, items) in redactions {
            let Some(cell) = cells
                .iter()
                .find(|c| c.start == loc.start_offset && c.end == loc.end_offset)
            else {
                continue;
            };
            let mut value = cell.value.clone();
            apply_text_redactions(&mut value, &items, TARGET)?;
            updates.push((cell.is_header, cell.row, cell.col, value));
        }

        for (is_header, row, col, new_value) in updates {
            if is_header {
                let headers = self
                    .data
                    .headers
                    .as_mut()
                    .ok_or_else(|| Error::validation("no headers to edit", TARGET))?;
                headers[col] = new_value;
            } else {
                let row_vec = self.data.rows.get_mut(row).ok_or_else(|| {
                    Error::validation(format!("row {row} out of bounds"), TARGET)
                })?;
                let target = row_vec.get_mut(col).ok_or_else(|| {
                    Error::validation(
                        format!("col {col} out of bounds in row {row}"),
                        TARGET,
                    )
                })?;
                *target = new_value;
            }
        }
        Ok(())
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

    /// Locate all cells by serializing and finding field boundaries.
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
            if line[pos..].starts_with('"') {
                let content_start = pos;
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
                return Some((content_start, pos));
            } else {
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
            pos += 1;
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
    use crate::transform::{ConflictPolicy, TextOutput};

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
    async fn locations_with_headers() {
        let h = handler_with_headers(
            vec!["name", "age"],
            vec![vec!["Alice", "30"], vec!["Bob", "25"]],
        );
        let items: Vec<_> = h.locations().collect().await;
        assert_eq!(items.len(), 6);
    }

    #[tokio::test]
    async fn locations_no_headers() {
        let h = handler_no_headers(vec![vec!["x", "y"], vec!["1", "2"]]);
        let items: Vec<_> = h.locations().collect().await;
        assert_eq!(items.len(), 4);
    }

    #[tokio::test]
    async fn redact_cell() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["ssn"], vec![vec!["123-45-6789"]]);
        let items: Vec<_> = h.locations().collect().await;
        let data_loc = items[1].location.clone();
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            data_loc,
            TextRedaction::new(0, 11, TextOutput::replace("[REDACTED]")),
        )
        .unwrap();
        h.redact(rs).await?;
        assert_eq!(h.cell(0, 0), Some("[REDACTED]"));
        Ok(())
    }

    #[tokio::test]
    async fn redact_header() -> Result<(), Error> {
        let mut h = handler_with_headers(vec!["secret_field"], vec![vec!["value"]]);
        let items: Vec<_> = h.locations().collect().await;
        let header_loc = items[0].location.clone();
        let mut rs = Redactions::new(ConflictPolicy::Reject);
        rs.try_insert(
            header_loc,
            TextRedaction::new(0, 12, TextOutput::replace("redacted")),
        )
        .unwrap();
        h.redact(rs).await?;
        assert_eq!(h.headers(), Some(["redacted".to_string()].as_slice()));
        Ok(())
    }

    #[tokio::test]
    async fn read_returns_cell() {
        let h = handler_with_headers(vec!["name"], vec![vec!["Alice"]]);
        let items: Vec<_> = h.locations().collect().await;
        assert_eq!(
            h.read(&items[1].location).await.unwrap().as_str(),
            "Alice"
        );
    }

    #[tokio::test]
    async fn quoted_field_offsets_correct() {
        let h = handler_with_headers(vec!["bio"], vec![vec!["has, comma"]]);
        let items: Vec<_> = h.locations().collect().await;
        // Header + data cell.
        assert_eq!(items.len(), 2);
        let data_loc = &items[1].location;
        assert!(data_loc.end_offset > data_loc.start_offset);
        assert_eq!(h.read(data_loc).await.unwrap().as_str(), "has, comma");
    }

    #[tokio::test]
    async fn empty_data_with_headers() {
        let h = handler_with_headers(vec!["a", "b"], vec![]);
        let items: Vec<_> = h.locations().collect().await;
        assert_eq!(items.len(), 2);
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
