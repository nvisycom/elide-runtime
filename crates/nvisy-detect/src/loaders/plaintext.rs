use async_trait::async_trait;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::Document;
use nvisy_core::errors::NvisyError;
use nvisy_core::traits::loader::{Loader, LoaderOutput};

pub struct PlaintextLoader;

#[async_trait]
impl Loader for PlaintextLoader {
    fn id(&self) -> &str {
        "plaintext"
    }

    fn extensions(&self) -> &[&str] {
        &["txt", "text"]
    }

    fn content_types(&self) -> &[&str] {
        &["text/plain"]
    }

    async fn load(
        &self,
        blob: &Blob,
        _params: &serde_json::Value,
    ) -> Result<Vec<LoaderOutput>, NvisyError> {
        let content = String::from_utf8(blob.content.to_vec()).map_err(|e| {
            NvisyError::validation(
                format!("Invalid UTF-8 in plaintext: {}", e),
                "plaintext-loader",
            )
        })?;
        let mut doc = Document::new(content);
        doc.source_format = Some("txt".to_string());
        doc.data.parent_id = Some(blob.data.id);
        Ok(vec![LoaderOutput::Document(doc)])
    }
}
