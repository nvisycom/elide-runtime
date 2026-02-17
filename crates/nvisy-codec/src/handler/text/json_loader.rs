//! JSON loader — validates and parses raw JSON content into a
//! [`Document<JsonHandler>`].
//!
//! The loader detects the indentation style and trailing-newline
//! convention of the source file so that [`JsonData`] preserves
//! whitespace for round-trip fidelity.

use std::num::NonZeroU32;

use nvisy_core::error::Error;
use nvisy_core::io::ContentData;

use crate::document::Document;
use crate::handler::{
    JsonData, JsonHandler, JsonIndent, Loader, TextEncoding,
};

/// Parameters for [`JsonLoader`].
#[derive(Debug, Default)]
pub struct JsonParams {
    /// Character encoding of the input bytes.
    pub encoding: TextEncoding,
}

/// Loader that validates and parses JSON files.
///
/// Produces a single [`Document<JsonHandler>`] per input.  The
/// loaded handler stores the parsed [`serde_json::Value`] tree
/// together with formatting metadata for round-trip fidelity.
#[derive(Debug)]
pub struct JsonLoader;

#[async_trait::async_trait]
impl Loader for JsonLoader {
    type Handler = JsonHandler;
    type Params = JsonParams;

    async fn load(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<Vec<Document<JsonHandler>>, Error> {
        let raw = content.to_bytes();
        let text = params.encoding.decode_bytes(&raw, "json-loader")?;
        let (indent, trailing_newline) = detect_formatting(&text);

        let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
            Error::validation(format!("Invalid JSON: {e}"), "json-loader")
        })?;

        let handler = JsonHandler {
            data: JsonData {
                value,
                indent,
                trailing_newline,
            },
        };
        let doc = Document::new(handler).with_parent(content);
        Ok(vec![doc])
    }
}

/// Detect indentation style and trailing newline from raw JSON source.
///
/// Inspects the first indented line to determine the whitespace
/// convention.  Falls back to [`JsonIndent::Compact`] when no
/// indentation is present (single-line JSON).
fn detect_formatting(source: &str) -> (JsonIndent, bool) {
    let trailing_newline = source.ends_with('\n');

    let indent = source
        .lines()
        .find_map(|line| {
            let stripped = line.trim_start();
            if stripped.len() == line.len() {
                return None;
            }
            let ws = &line[..line.len() - stripped.len()];
            if ws.starts_with('\t') {
                Some(JsonIndent::Tab)
            } else {
                let n = u32::try_from(ws.len()).unwrap_or(u32::MAX);
                Some(JsonIndent::Spaces(
                    NonZeroU32::new(n).unwrap_or(NonZeroU32::new(2).unwrap()),
                ))
            }
        })
        .unwrap_or(JsonIndent::Compact);

    (indent, trailing_newline)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use nvisy_core::path::ContentSource;
    use nvisy_core::fs::DocumentType;
    use serde_json::json;

    fn content_from_str(s: &str) -> ContentData {
        ContentData::new(ContentSource::new(), Bytes::from(s.to_owned()))
    }

    #[tokio::test]
    async fn load_simple_object() {
        let content = content_from_str(r#"{"name": "Alice", "age": 30}"#);
        let docs = JsonLoader
            .load(&content, &JsonParams::default())
            .await
            .unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].document_type(), DocumentType::Json);

        let handler = docs[0].handler();
        assert_eq!(handler.value(), &json!({"name": "Alice", "age": 30}));
    }

    #[tokio::test]
    async fn load_detects_compact_formatting() {
        let content = content_from_str(r#"{"a":1}"#);
        let docs = JsonLoader
            .load(&content, &JsonParams::default())
            .await
            .unwrap();
        let h = docs[0].handler();
        assert_eq!(h.indent(), JsonIndent::Compact);
        assert!(!h.trailing_newline());
    }

    #[tokio::test]
    async fn load_detects_two_space_indent() {
        let content = content_from_str("{\n  \"a\": 1\n}\n");
        let docs = JsonLoader
            .load(&content, &JsonParams::default())
            .await
            .unwrap();
        let h = docs[0].handler();
        assert_eq!(h.indent(), JsonIndent::two_spaces());
        assert!(h.trailing_newline());
    }

    #[tokio::test]
    async fn load_detects_four_space_indent() {
        let content = content_from_str("{\n    \"a\": 1\n}\n");
        let docs = JsonLoader
            .load(&content, &JsonParams::default())
            .await
            .unwrap();
        assert_eq!(docs[0].handler().indent(), JsonIndent::four_spaces());
    }

    #[tokio::test]
    async fn load_detects_tab_indent() {
        let content = content_from_str("{\n\t\"a\": 1\n}\n");
        let docs = JsonLoader
            .load(&content, &JsonParams::default())
            .await
            .unwrap();
        assert_eq!(docs[0].handler().indent(), JsonIndent::Tab);
    }

    #[tokio::test]
    async fn load_invalid_utf8() {
        let content = ContentData::new(
            ContentSource::new(),
            Bytes::from_static(&[0xFF, 0xFE, 0x00]),
        );
        let err = JsonLoader
            .load(&content, &JsonParams::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("UTF-8"));
    }

    #[tokio::test]
    async fn load_invalid_json() {
        let content = content_from_str("{not json}");
        let err = JsonLoader
            .load(&content, &JsonParams::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("JSON"));
    }
}
