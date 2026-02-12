//! Plain-text file loader.

use nvisy_core::io::ContentData;
use nvisy_core::error::Error;

use crate::document::Document;
use crate::handler::{PlaintextHandler, FormatHandler, TextLoader};

/// Loads plain-text content into a single [`Document`].
pub struct PlaintextLoader;

impl Clone for PlaintextLoader {
    fn clone(&self) -> Self { Self }
}

#[async_trait::async_trait]
impl TextLoader for PlaintextLoader {
    type Params = ();

    async fn load(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<Vec<Document<FormatHandler>>, Error> {
        let text = String::from_utf8(content.to_bytes().to_vec()).map_err(|e| {
            Error::validation(
                format!("Invalid UTF-8 in plaintext: {}", e),
                "plaintext-loader",
            )
        })?;
        let mut doc = Document::new(PlaintextHandler).with_text(text);
        doc.source.set_parent_id(Some(content.content_source.as_uuid()));
        Ok(vec![doc.into_format()])
    }
}

impl crate::handler::Handler for PlaintextLoader {
    fn id(&self) -> &str { PlaintextHandler.id() }
    fn extensions(&self) -> &[&str] { PlaintextHandler.extensions() }
    fn content_types(&self) -> &[&str] { PlaintextHandler.content_types() }
}
