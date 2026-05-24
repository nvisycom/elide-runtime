//! JSON loader: validates and parses raw JSON content into a
//! [`JsonHandler`].
//!
//! The loader detects the indentation style and trailing-newline
//! convention of the source file so that [`JsonData`] preserves
//! whitespace for round-trip fidelity.

use std::num::NonZeroU32;

use nvisy_codec::handler::Loader;
use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource, TextEncoding};

use super::{JsonData, JsonHandler, JsonIndent};

/// Parameters for [`JsonLoader`].
#[derive(Debug, Default)]
pub struct JsonParams {
    /// Character encoding of the input bytes.
    pub encoding: TextEncoding,
}

/// Loader that validates and parses JSON files.
///
/// Produces a single [`JsonHandler`] per input.  The
/// loaded handler stores the parsed [`Value`] tree
/// together with formatting metadata for round-trip fidelity.
///
/// [`Value`]: serde_json::Value
#[derive(Debug, Default)]
pub struct JsonLoader;

#[async_trait::async_trait]
impl Loader for JsonLoader {
    type Handler = JsonHandler;
    type Params = JsonParams;

    #[tracing::instrument(name = "json.decode", skip_all, fields(input_bytes))]
    async fn decode(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<JsonHandler, Error> {
        let raw = content.to_bytes();
        tracing::Span::current().record("input_bytes", raw.len());
        let text = params.encoding.decode_bytes(&raw, "json-loader")?;
        let (indent, trailing_newline) = detect_formatting(&text);

        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| Error::validation(format!("Invalid JSON: {e}"), "json-loader"))?;

        let source = ContentSource::new().with_parent(&content.content_source);
        let handler = JsonHandler::new(JsonData {
            value,
            indent,
            trailing_newline,
        })
        .with_source(source);
        Ok(handler)
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
    use bytes::Bytes;
    use nvisy_codec::handler::Handler;
    use nvisy_core::Error;
    use nvisy_core::content::ContentSource;
    use nvisy_core::media::{DocumentType, TextFormat};
    use serde_json::json;

    use super::*;

    fn content_from_str(s: &str) -> ContentData {
        ContentData::new(ContentSource::new(), Bytes::from(s.to_owned()))
    }

    #[tokio::test]
    async fn load_simple_object() -> Result<(), Error> {
        let content = content_from_str(r#"{"name": "Alice", "age": 30}"#);
        let doc = JsonLoader.decode(&content, &JsonParams::default()).await?;

        assert_eq!(doc.document_type(), DocumentType::Text(TextFormat::Json));
        assert_eq!(doc.value(), &json!({"name": "Alice", "age": 30}));
        Ok(())
    }

    #[tokio::test]
    async fn load_detects_compact_formatting() -> Result<(), Error> {
        let content = content_from_str(r#"{"a":1}"#);
        let doc = JsonLoader.decode(&content, &JsonParams::default()).await?;
        assert_eq!(doc.indent(), JsonIndent::Compact);
        assert!(!doc.trailing_newline());
        Ok(())
    }

    #[tokio::test]
    async fn load_detects_two_space_indent() -> Result<(), Error> {
        let content = content_from_str("{\n  \"a\": 1\n}\n");
        let doc = JsonLoader.decode(&content, &JsonParams::default()).await?;
        assert_eq!(doc.indent(), JsonIndent::two_spaces());
        assert!(doc.trailing_newline());
        Ok(())
    }

    #[tokio::test]
    async fn load_detects_tab_indent() -> Result<(), Error> {
        let content = content_from_str("{\n\t\"a\": 1\n}\n");
        let doc = JsonLoader.decode(&content, &JsonParams::default()).await?;
        assert_eq!(doc.indent(), JsonIndent::Tab);
        Ok(())
    }

    #[tokio::test]
    async fn load_invalid_json() {
        let content = content_from_str("{not json}");
        let err = JsonLoader
            .decode(&content, &JsonParams::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("JSON"));
    }
}
