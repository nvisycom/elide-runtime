//! CSV loader: validates and parses raw CSV content into a
//! [`CsvHandler`].
//!
//! The loader auto-detects the field delimiter (comma, tab, semicolon,
//! pipe) by inspecting the first line.

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource, TextEncoding};

use crate::handler::{CsvData, CsvHandler, Loader};

const TARGET: &str = "nvisy_codec::handler::csv";

/// Parameters for [`CsvLoader`].
#[derive(Debug)]
pub struct CsvParams {
    /// Character encoding of the input bytes.
    pub encoding: TextEncoding,
    /// Whether the first row contains column headers.
    /// Defaults to `true`.
    pub has_headers: bool,
    /// Override the field delimiter. If `None`, the loader will
    /// auto-detect from the first line.
    pub delimiter: Option<u8>,
}

impl Default for CsvParams {
    fn default() -> Self {
        Self {
            encoding: TextEncoding::Utf8,
            has_headers: true,
            delimiter: None,
        }
    }
}

/// Loader that validates and parses CSV files.
///
/// Produces a single [`CsvHandler`] per input.
#[derive(Debug, Default)]
pub struct CsvLoader;

#[async_trait::async_trait]
impl Loader for CsvLoader {
    type Handler = CsvHandler;
    type Params = CsvParams;

    #[tracing::instrument(name = "csv.decode", skip_all, fields(input_bytes, rows, delimiter))]
    async fn decode(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<CsvHandler, Error> {
        let raw = content.to_bytes();
        tracing::Span::current().record("input_bytes", raw.len());
        let text = params.encoding.decode_bytes(&raw, "csv-loader")?;
        let trailing_newline = text.ends_with('\n');
        let delimiter = params.delimiter.unwrap_or_else(|| detect_delimiter(&text));
        tracing::Span::current().record("delimiter", tracing::field::display(delimiter as char));

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(params.has_headers)
            .delimiter(delimiter)
            .flexible(true)
            .from_reader(text.as_bytes());

        let headers = if params.has_headers {
            let hdr = reader
                .headers()
                .map_err(|e| Error::validation(format!("CSV header error: {e}"), "csv-loader"))?;
            Some(hdr.iter().map(String::from).collect())
        } else {
            None
        };

        let mut rows = Vec::new();
        for result in reader.records() {
            let record = result
                .map_err(|e| Error::validation(format!("CSV parse error: {e}"), "csv-loader"))?;
            rows.push(record.iter().map(String::from).collect());
        }

        tracing::Span::current().record("rows", rows.len());
        let source = ContentSource::new().with_parent(&content.content_source);
        let handler = CsvHandler::new(CsvData {
            headers,
            rows,
            delimiter,
            trailing_newline,
        })
        .with_source(source);
        Ok(handler)
    }
}

/// Auto-detect the CSV delimiter by sampling up to 5 lines and
/// picking the candidate with the highest, most consistent count.
///
/// Tie-break: prefer comma.
fn detect_delimiter(text: &str) -> u8 {
    let candidates: &[(u8, char)] = &[(b',', ','), (b'\t', '\t'), (b';', ';'), (b'|', '|')];

    let sample_lines: Vec<&str> = text.lines().take(5).collect();
    if sample_lines.is_empty() {
        return b',';
    }

    let mut best_byte = b',';
    let mut best_score = (0usize, 0usize); // (min_count, total_count) — higher is better

    for &(byte, ch) in candidates {
        let counts: Vec<usize> = sample_lines
            .iter()
            .map(|line| line.matches(ch).count())
            .collect();
        let total: usize = counts.iter().sum();
        let min = counts.iter().copied().min().unwrap_or(0);

        // Prefer the candidate with the highest minimum per-line count
        // (consistency), then highest total. Comma wins ties.
        let score = (min, total);
        if score > best_score || (score == best_score && byte == b',') {
            best_score = score;
            best_byte = byte;
        }
    }

    if best_byte != b',' {
        tracing::debug!(target: TARGET, delimiter = %char::from(best_byte), "detected non-comma CSV delimiter");
    }
    best_byte
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use futures::StreamExt;
    use nvisy_core::Error;
    use nvisy_core::content::ContentSource;
    use nvisy_core::media::{DocumentType, SpreadsheetFormat};

    use super::*;
    use crate::handler::{Handler, TextHandler};

    fn content_from_str(s: &str) -> ContentData {
        ContentData::new(ContentSource::new(), Bytes::from(s.to_owned()))
    }

    #[tokio::test]
    async fn load_with_headers() -> Result<(), Error> {
        let content = content_from_str("name,age\nAlice,30\nBob,25\n");
        let doc = CsvLoader.decode(&content, &CsvParams::default()).await?;

        assert_eq!(
            doc.document_type(),
            DocumentType::Spreadsheet(SpreadsheetFormat::Csv)
        );
        assert_eq!(
            doc.headers(),
            Some(["name", "age"].map(String::from).as_slice())
        );
        assert_eq!(doc.len(), 2);
        assert_eq!(doc.cell(0, 0), Some("Alice"));
        assert_eq!(doc.cell(1, 1), Some("25"));
        assert!(doc.trailing_newline());
        Ok(())
    }

    #[tokio::test]
    async fn load_without_headers() -> Result<(), Error> {
        let params = CsvParams {
            has_headers: false,
            ..CsvParams::default()
        };
        let content = content_from_str("x,y\n1,2\n");
        let doc = CsvLoader.decode(&content, &params).await?;

        assert!(doc.headers().is_none());
        assert_eq!(doc.len(), 2);
        assert_eq!(doc.cell(0, 0), Some("x"));
        Ok(())
    }

    #[tokio::test]
    async fn load_tab_delimited() -> Result<(), Error> {
        let content = content_from_str("a\tb\n1\t2\n");
        let doc = CsvLoader.decode(&content, &CsvParams::default()).await?;
        assert_eq!(doc.delimiter(), b'\t');
        assert_eq!(doc.headers(), Some(["a", "b"].map(String::from).as_slice()));
        Ok(())
    }

    #[tokio::test]
    async fn load_semicolon_delimited() -> Result<(), Error> {
        let content = content_from_str("a;b\n1;2\n");
        let doc = CsvLoader.decode(&content, &CsvParams::default()).await?;
        assert_eq!(doc.delimiter(), b';');
        Ok(())
    }

    #[tokio::test]
    async fn load_quoted_fields() -> Result<(), Error> {
        let content = content_from_str("name,bio\n\"Alice\",\"Has a, comma\"\n");
        let doc = CsvLoader.decode(&content, &CsvParams::default()).await?;
        assert_eq!(doc.cell(0, 1), Some("Has a, comma"));
        Ok(())
    }

    #[tokio::test]
    async fn load_spans_round_trip() -> Result<(), Error> {
        let content = content_from_str("name,age\nAlice,30\n");
        let doc = CsvLoader.decode(&content, &CsvParams::default()).await?;
        let spans: Vec<_> = doc.text_spans().await.collect().await;

        // 2 header + 2 data
        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0].data, "name");
        assert_eq!(spans[1].data, "age");
        assert_eq!(spans[2].data, "Alice");
        assert_eq!(spans[3].data, "30");
        Ok(())
    }

    #[tokio::test]
    async fn load_invalid_utf8() {
        let content = ContentData::new(
            ContentSource::new(),
            Bytes::from_static(&[0xFF, 0xFE, 0x00]),
        );
        let err = CsvLoader
            .decode(&content, &CsvParams::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("UTF-8"));
    }

    // --- detect_delimiter unit tests ---

    #[test]
    fn detect_tab_delimited() {
        let text = "a\tb\tc\n1\t2\t3\n4\t5\t6\n";
        assert_eq!(detect_delimiter(text), b'\t');
    }

    #[test]
    fn detect_semicolons_with_commas_in_content() {
        // Commas appear inside values but semicolons are the real delimiter.
        let text = "\"a,b\";c;d\n\"e,f\";g;h\n";
        assert_eq!(detect_delimiter(text), b';');
    }

    #[test]
    fn detect_no_delimiters_defaults_to_comma() {
        let text = "just plain text\nno delimiters here\n";
        assert_eq!(detect_delimiter(text), b',');
    }
}
