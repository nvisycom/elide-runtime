//! CSV loader — validates and parses raw CSV content into a
//! [`Document<CsvHandler>`].
//!
//! The loader auto-detects the field delimiter (comma, tab, semicolon,
//! pipe) by inspecting the first line.

use nvisy_core::error::Error;
use nvisy_core::io::ContentData;

use crate::document::Document;
use crate::handler::{CsvData, CsvHandler, Loader, TextEncoding};

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
/// Produces a single [`Document<CsvHandler>`] per input.
#[derive(Debug)]
pub struct CsvLoader;

#[async_trait::async_trait]
impl Loader for CsvLoader {
    type Handler = CsvHandler;
    type Params = CsvParams;

    async fn load(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<Vec<Document<CsvHandler>>, Error> {
        let raw = content.to_bytes();
        let text = params.encoding.decode_bytes(&raw, "csv-loader")?;
        let trailing_newline = text.ends_with('\n');
        let delimiter = params
            .delimiter
            .unwrap_or_else(|| detect_delimiter(&text));

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(params.has_headers)
            .delimiter(delimiter)
            .flexible(true)
            .from_reader(text.as_bytes());

        let headers = if params.has_headers {
            let hdr = reader.headers().map_err(|e| {
                Error::validation(format!("CSV header error: {e}"), "csv-loader")
            })?;
            Some(hdr.iter().map(String::from).collect())
        } else {
            None
        };

        let mut rows = Vec::new();
        for result in reader.records() {
            let record = result.map_err(|e| {
                Error::validation(format!("CSV parse error: {e}"), "csv-loader")
            })?;
            rows.push(record.iter().map(String::from).collect());
        }

        let handler = CsvHandler {
            data: CsvData {
                headers,
                rows,
                delimiter,
                trailing_newline,
            },
        };
        let doc = Document::new(handler).with_parent(content);
        Ok(vec![doc])
    }
}

/// Auto-detect the CSV delimiter by counting candidate characters
/// in the first line.
fn detect_delimiter(text: &str) -> u8 {
    let first_line = text.lines().next().unwrap_or("");
    let candidates: &[(u8, char)] = &[
        (b',', ','),
        (b'\t', '\t'),
        (b';', ';'),
        (b'|', '|'),
    ];
    candidates
        .iter()
        .max_by_key(|(_, ch)| first_line.matches(*ch).count())
        .map(|(b, _)| *b)
        .unwrap_or(b',')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::Handler;
    use bytes::Bytes;
    use futures::StreamExt;
    use nvisy_core::path::ContentSource;
    use nvisy_core::fs::DocumentType;

    fn content_from_str(s: &str) -> ContentData {
        ContentData::new(ContentSource::new(), Bytes::from(s.to_owned()))
    }

    #[tokio::test]
    async fn load_with_headers() {
        let content = content_from_str("name,age\nAlice,30\nBob,25\n");
        let docs = CsvLoader
            .load(&content, &CsvParams::default())
            .await
            .unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].document_type(), DocumentType::Csv);

        let h = docs[0].handler();
        assert_eq!(h.headers(), Some(["name", "age"].map(String::from).as_slice()));
        assert_eq!(h.row_count(), 2);
        assert_eq!(h.cell(0, 0), Some("Alice"));
        assert_eq!(h.cell(1, 1), Some("25"));
        assert!(h.trailing_newline());
    }

    #[tokio::test]
    async fn load_without_headers() {
        let params = CsvParams {
            has_headers: false,
            ..CsvParams::default()
        };
        let content = content_from_str("x,y\n1,2\n");
        let docs = CsvLoader.load(&content, &params).await.unwrap();

        let h = docs[0].handler();
        assert!(h.headers().is_none());
        assert_eq!(h.row_count(), 2);
        assert_eq!(h.cell(0, 0), Some("x"));
    }

    #[tokio::test]
    async fn load_tab_delimited() {
        let content = content_from_str("a\tb\n1\t2\n");
        let docs = CsvLoader
            .load(&content, &CsvParams::default())
            .await
            .unwrap();
        let h = docs[0].handler();
        assert_eq!(h.delimiter(), b'\t');
        assert_eq!(h.headers(), Some(["a", "b"].map(String::from).as_slice()));
    }

    #[tokio::test]
    async fn load_semicolon_delimited() {
        let content = content_from_str("a;b\n1;2\n");
        let docs = CsvLoader
            .load(&content, &CsvParams::default())
            .await
            .unwrap();
        assert_eq!(docs[0].handler().delimiter(), b';');
    }

    #[tokio::test]
    async fn load_quoted_fields() {
        let content = content_from_str("name,bio\n\"Alice\",\"Has a, comma\"\n");
        let docs = CsvLoader
            .load(&content, &CsvParams::default())
            .await
            .unwrap();
        let h = docs[0].handler();
        assert_eq!(h.cell(0, 1), Some("Has a, comma"));
    }

    #[tokio::test]
    async fn load_empty() {
        let content = content_from_str("");
        let docs = CsvLoader
            .load(&content, &CsvParams::default())
            .await
            .unwrap();
        let h = docs[0].handler();
        assert_eq!(h.row_count(), 0);
    }

    #[tokio::test]
    async fn load_spans_round_trip() {
        let content = content_from_str("name,age\nAlice,30\n");
        let docs = CsvLoader
            .load(&content, &CsvParams::default())
            .await
            .unwrap();
        let spans: Vec<_> = docs[0].handler().view_spans().await.collect().await;

        // 2 header + 2 data
        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0].data, "name");
        assert_eq!(spans[1].data, "age");
        assert_eq!(spans[2].data, "Alice");
        assert_eq!(spans[2].id.key, "name");
        assert_eq!(spans[3].data, "30");
        assert_eq!(spans[3].id.key, "age");
    }

    #[tokio::test]
    async fn load_invalid_utf8() {
        let content = ContentData::new(
            ContentSource::new(),
            Bytes::from_static(&[0xFF, 0xFE, 0x00]),
        );
        let err = CsvLoader
            .load(&content, &CsvParams::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("UTF-8"));
    }
}
