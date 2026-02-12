//! CSV file loader.

use nvisy_core::io::ContentData;
use nvisy_core::error::Error;

use crate::document::Document;
use crate::handler::{CsvHandler, FormatHandler, TextLoader};

/// Loads CSV content into a single [`Document`] containing the raw CSV text.
pub struct CsvLoader;

impl Clone for CsvLoader {
    fn clone(&self) -> Self { Self }
}

#[async_trait::async_trait]
impl TextLoader for CsvLoader {
    type Params = ();

    async fn load(
        &self,
        content: &ContentData,
        _params: &Self::Params,
    ) -> Result<Vec<Document<FormatHandler>>, Error> {
        let text = String::from_utf8(content.to_bytes().to_vec()).map_err(|e| {
            Error::validation(format!("Invalid UTF-8 in CSV: {}", e), "csv-loader")
        })?;
        let mut doc = Document::new(CsvHandler).with_text(text);
        doc.source.set_parent_id(Some(content.content_source.as_uuid()));
        Ok(vec![doc.into_format()])
    }
}

impl crate::handler::Handler for CsvLoader {
    fn id(&self) -> &str { CsvHandler.id() }
    fn extensions(&self) -> &[&str] { CsvHandler.extensions() }
    fn content_types(&self) -> &[&str] { CsvHandler.content_types() }
}
