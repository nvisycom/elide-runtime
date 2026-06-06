//! JSON loader: validates and parses raw JSON content into a
//! [`JsonHandler`].
//!
//! Detects the indentation style and trailing-newline convention of
//! the source so [`JsonData`] preserves whitespace for round-trip
//! fidelity.

use std::num::NonZeroU32;

use async_trait::async_trait;
use nvisy_core::Error;
use nvisy_core::modality::Text;

use super::{JsonData, JsonHandler, JsonIndent};
use crate::content::{ContentData, ContentSource, TextEncoding};
use crate::core::Loader;

/// Loader for JSON files. Produces one [`JsonHandler`] per input.
#[derive(Debug, Default)]
pub struct JsonLoader {
    /// Character encoding of the input bytes. Defaults to UTF-8.
    pub encoding: TextEncoding,
}

#[async_trait]
impl Loader<Text> for JsonLoader {
    type Handler = JsonHandler;

    #[tracing::instrument(name = "json.decode", skip_all, fields(input_bytes))]
    async fn decode(&self, content: ContentData) -> Result<JsonHandler, Error> {
        let parent = content.content_source;
        let raw = content.to_bytes();
        tracing::Span::current().record("input_bytes", raw.len());
        let text = self.encoding.decode_bytes(&raw, "json-loader")?;
        let (indent, trailing_newline) = detect_formatting(&text);
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| Error::validation(format!("Invalid JSON: {e}"), "json-loader"))?;

        let source = ContentSource::new().with_parent(&parent);
        Ok(JsonHandler::new(JsonData {
            value,
            indent,
            trailing_newline,
        })
        .with_source(source))
    }
}

/// Detect indentation style and trailing newline from raw JSON source.
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
