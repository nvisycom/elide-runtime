//! JSON file loader.

use nvisy_core::io::ContentData;
use nvisy_core::error::Error;

use crate::document::Document;
use crate::handler::{JsonHandler, FormatHandler, TextLoader};

/// Loads JSON content into a single [`Document`] containing the raw JSON text.
pub struct JsonLoader;

impl Clone for JsonLoader {
    fn clone(&self) -> Self { Self }
}

#[async_trait::async_trait]
impl TextLoader for JsonLoader {
    type Params = ();

    async fn load(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<Vec<Document<FormatHandler>>, Error> {
        let text = String::from_utf8(content.to_bytes().to_vec()).map_err(|e| {
            Error::validation(format!("Invalid UTF-8 in JSON: {}", e), "json-loader")
        })?;
        // Validate it's valid JSON
        let _: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
            Error::validation(format!("Invalid JSON: {}", e), "json-loader")
        })?;
        let mut doc = Document::new(JsonHandler).with_text(text);
        doc.source.set_parent_id(Some(content.content_source.as_uuid()));
        Ok(vec![doc.into_format()])
    }
}

impl crate::handler::Handler for JsonLoader {
    fn id(&self) -> &str { JsonHandler.id() }
    fn extensions(&self) -> &[&str] { JsonHandler.extensions() }
    fn content_types(&self) -> &[&str] { JsonHandler.content_types() }
}
