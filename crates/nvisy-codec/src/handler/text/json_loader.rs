//! JSON loader: decode source bytes and hand them to
//! [`JsonHandler`] verbatim. Formatting (indentation, key order,
//! trailing whitespace) is preserved by the handler's slot model;
//! the loader only does encoding + well-formedness checks.

use nvisy_core::Error;
use nvisy_core::modality::Text;

use super::JsonHandler;
use crate::content::{ContentData, ContentSource, TextEncoding};
use crate::core::Loader;

/// Loader for JSON files. Produces one [`JsonHandler`] per input.
#[derive(Debug, Default)]
pub struct JsonLoader {
    /// Character encoding of the input bytes. Defaults to UTF-8.
    pub encoding: TextEncoding,
}

#[async_trait::async_trait]
impl Loader<Text> for JsonLoader {
    type Handler = JsonHandler;

    #[tracing::instrument(name = "json.decode", skip_all, fields(input_bytes))]
    async fn decode(&self, content: ContentData) -> Result<JsonHandler, Error> {
        let parent = content.content_source;
        let raw = content.to_bytes();
        tracing::Span::current().record("input_bytes", raw.len());
        let text = self.encoding.decode_bytes(&raw, "json-loader")?;
        // Validate well-formedness eagerly; the handler's lexer
        // re-parses but with a friendlier error path. Reject here so
        // callers get a single decode-time validation point.
        serde_json::from_str::<serde_json::Value>(&text)
            .map_err(|e| Error::validation(format!("Invalid JSON: {e}"), "json-loader"))?;
        let source = ContentSource::new().with_parent(&parent);
        Ok(JsonHandler::from_source_string(text).with_source(source))
    }
}
