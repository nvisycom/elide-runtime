use async_trait::async_trait;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::Document;
use nvisy_core::errors::NvisyError;
use nvisy_core::traits::loader::{Loader, LoaderOutput};

pub struct JsonLoader;

#[async_trait]
impl Loader for JsonLoader {
    fn id(&self) -> &str {
        "json"
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }

    fn content_types(&self) -> &[&str] {
        &["application/json"]
    }

    async fn load(
        &self,
        blob: &Blob,
        _params: &serde_json::Value,
    ) -> Result<Vec<LoaderOutput>, NvisyError> {
        let content = String::from_utf8(blob.content.to_vec()).map_err(|e| {
            NvisyError::validation(format!("Invalid UTF-8 in JSON: {}", e), "json-loader")
        })?;
        // Validate it's valid JSON
        let _: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
            NvisyError::validation(format!("Invalid JSON: {}", e), "json-loader")
        })?;
        let mut doc = Document::new(content);
        doc.source_format = Some("json".to_string());
        doc.data.parent_id = Some(blob.data.id);
        Ok(vec![LoaderOutput::Document(doc)])
    }
}
